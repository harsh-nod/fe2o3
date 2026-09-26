use super::layout_tests::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

#[test]
fn original_closure_foreign_ledger_cannot_consume_or_refund_the_original_owner() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(313).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    layouts.select_original_closure(&owner, RECURSIVE, &mut budget).unwrap();
    let resident = budget.storage();
    let retained = layouts.lease.persistent.get();
    let rows = layouts.physical.borrow().rows.len();
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut other = Budget::new(&mut other_work, 64 * 1024 * 1024);
    other.reserve_storage(resident).unwrap();
    let first = layouts.select_original_closure(&owner, RECURSIVE, &mut other).unwrap_err();
    assert!(matches!(resource(first), ArgumentResourceV1::Accounting));
    assert_eq!(other.storage(), resident);
    assert_eq!(other.work(), 0);
    assert_eq!(budget.storage(), resident);
    assert_eq!(layouts.lease.persistent.get(), retained);
    assert_eq!(layouts.physical.borrow().rows.len(), rows);
    let before = budget.work();
    assert!(matches!(resource(layouts.select_original_closure(&owner, RECURSIVE, &mut budget).unwrap_err()), ArgumentResourceV1::Accounting));
    assert_eq!(budget.work(), before);
    assert!(layouts.release(&mut budget).is_err());
    assert_eq!(budget.storage(), 313);
    assert_eq!(other.storage(), resident);
    other.release_storage(resident).unwrap();
}

#[test]
fn original_extension_owner_header_has_an_independent_field_and_alignment_mirror() {
    #[allow(dead_code)]
    struct PhysicalFields {
        rows: Vec<StorageLayoutV1>,
        interner: BTreeMap<(RowKey, u64), Vec<StorageLayoutIdV1>>,
        source_keys: Vec<RowKey>,
        containment_depths: Vec<usize>,
        edge_count: usize,
        original_index: BTreeMap<RowKey, StorageLayoutIdV1>,
    }
    assert_eq!(size_of::<PhysicalFields>(), size_of::<SourceStoragePhysicalV29>());
    assert_eq!(std::mem::align_of::<PhysicalFields>(), std::mem::align_of::<SourceStoragePhysicalV29>());
}

#[test]
fn original_closure_cache_has_independent_exact_work_without_table_or_scratch_growth() {
    const WORK: usize = 20_000_000;
    const LIMIT: usize = 64 * 1024 * 1024;
    for original in [false, true] {
        // Original: owner1 + checked1 + one-key original search1.
        // Extended: owner1 + checked1 + one-key extension lookup32 + key1
        // + exact scalar row construction1 + exact shallow comparison1.
        let exact = if original { 3 } else { 37 };
        for short in [false, true] {
            let owner = owner();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(277).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, if original { &[WORD] } else { &[] }, &mut budget).unwrap();
            let schema = layouts.select_original_closure(&owner, WORD, &mut budget).unwrap();
            let count = layouts.physical.borrow().rows.len();
            let persistent = layouts.lease.persistent.get();
            let pressure = LIMIT - budget.storage();
            budget.reserve_storage(pressure).unwrap();
            assert!(budget.charge_work(usize::MAX).is_err());
            budget.charge_work(WORK - budget.work() - exact + usize::from(short)).unwrap();
            let before = budget.work();
            let result = layouts.select_original_closure(&owner, WORD, &mut budget);
            if short { assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Work(_))); }
            else { assert_eq!(result.unwrap(), schema); }
            assert_eq!(budget.work() - before, exact - usize::from(short));
            assert_eq!(budget.storage(), LIMIT);
            assert_eq!(layouts.lease.persistent.get(), persistent);
            assert_eq!(layouts.physical.borrow().rows.len(), count);
            assert_eq!(layouts.release(&mut budget).is_err(), short);
            assert_eq!(budget.storage(), 277 + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), 277);
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }
}

#[test]
fn original_containment_and_scc_walks_have_independent_exact_work() {
    const WORK: usize = 20_000_000;
    for components in [false, true] {
        // Two original nodes and two edges. Ordered index: header1 + Vec3
        // + two nodes + two edges = 8. Containment adds Vec3/init2/Vec3,
        // two roots and four frame visits: total22. SCC adds seven Vec3,
        // init6, two roots, four frames and two pops: total43.
        let exact = if components { 43 } else { 22 };
        for short in [false, true] {
            let owner = owner();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(281).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
            let mut graph = SourceStorageOriginalGraphV29::default();
            graph.discover(&layouts, RowKey::ty(RECURSIVE), &mut budget).unwrap();
            assert_eq!((graph.nodes.len(), graph.edges.len()), (2, 2));
            if components { graph.containment(&layouts, &mut budget).unwrap(); }
            assert!(budget.charge_work(usize::MAX).is_err());
            budget.charge_work(WORK - budget.work() - exact + usize::from(short)).unwrap();
            let before = budget.work();
            let result = if components {
                graph.components(&layouts, &mut budget).map(|scc| {
                    assert_eq!(scc.members.len(), 2);
                    assert_eq!(scc.ranges, [0..2]);
                })
            } else { graph.containment(&layouts, &mut budget) };
            if short { assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Work(_))); }
            else { result.unwrap(); }
            assert_eq!(budget.work() - before, exact - usize::from(short));
            assert!(layouts.physical.borrow().rows.is_empty());
            drop(graph);
            assert_eq!(layouts.release(&mut budget).is_err(), short);
            assert_eq!(budget.storage(), 281);
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }
}

#[test]
fn original_recursive_publication_has_independent_exact_persistent_growth_and_one_short_refusal() {
    const LIMIT: usize = 64 * 1024 * 1024;
    // Three table vectors grow to four slots. Each of two source keys gets
    // a 4-ID collision bucket and two 32-record map-path levels in each index.
    let map = 2 * 32 * size_of::<(RowKey, StorageLayoutIdV1, usize)>();
    let growth = 4 * (size_of::<StorageLayoutV1>() + size_of::<RowKey>() + size_of::<usize>())
        + 8 * size_of::<StorageLayoutIdV1>()
        + 4 * 32 * size_of::<((RowKey, u64), Vec<StorageLayoutIdV1>, usize)>() + 2 * map;
    for short in [false, true] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(283).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let mut graph = SourceStorageOriginalGraphV29::default();
        graph.discover(&layouts, RowKey::ty(RECURSIVE), &mut budget).unwrap();
        graph.containment(&layouts, &mut budget).unwrap();
        let components = graph.components(&layouts, &mut budget).unwrap();
        assert_eq!(components.ranges, [0..2]);
        let mut order = layouts.lease.vector(2, &mut budget).unwrap();
        for &node in &components.members {
            order.push((graph.nodes[node].depth, graph.nodes[node].key, node));
        }
        sort_rows(&mut order, &layouts.lease, &mut budget).unwrap();
        let mut staged = layouts.lease.vector(2, &mut budget).unwrap();
        let base = layouts.original_stage_group(&mut graph, &order, &mut staged, &mut budget).unwrap();
        assert_eq!(base, 0);
        assert_eq!(staged.iter().map(|row| row.backing).sum::<usize>(), size_of::<StorageFieldV1>());
        let resident = budget.storage();
        let retained = layouts.lease.persistent.get();
        let pressure = LIMIT - resident - growth + usize::from(short);
        budget.reserve_storage(pressure).unwrap();
        assert!(budget.reserve_storage(LIMIT + 1).is_err());
        let history = budget.failed_storage();
        let result = layouts.original_publish_group(&graph, &order, base, &mut staged, &mut budget);
        if short {
            assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Storage(_)));
            assert_eq!(staged.len(), 2);
            let physical = layouts.physical.borrow();
            assert!(physical.rows.is_empty());
            assert!(physical.original_extensions.is_empty());
            assert_eq!(physical.selected.len(), 2);
            assert!(physical.selected.values().all(Vec::is_empty));
            assert_eq!(layouts.lease.persistent.get() - retained, growth - map);
            assert_eq!(budget.storage(), resident + pressure + growth - map);
        } else {
            result.unwrap();
            assert!(staged.is_empty());
            assert_eq!(layouts.physical.borrow().rows.len(), 2);
            assert_eq!(layouts.physical.borrow().original_extensions.len(), 2);
            assert_eq!(layouts.lease.persistent.get() - retained, growth + size_of::<StorageFieldV1>());
            assert_eq!(budget.storage(), LIMIT);
        }
        assert_eq!(budget.failed_storage(), history);
        drop((graph, components, order, staged));
        assert_eq!(layouts.release(&mut budget).is_err(), short);
        assert_eq!(budget.storage(), 283 + pressure);
        budget.release_storage(pressure).unwrap();
        assert_eq!(budget.storage(), 283);
    }
}

#[test]
fn original_closure_limits_are_exact_for_a_recursive_pair() {
    for cut in 0..5 {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(293).unwrap();
        let limits = fe2o3_kernel_ir::StorageLayoutLimitsV1 {
            rows: 2 - usize::from(cut == 1), edges: 2 - usize::from(cut == 2),
            containment_depth: 2 - usize::from(cut == 3), object_bytes: 8 - u64::from(cut == 4),
        };
        let layouts = SourceStorageLayoutsV29::new_with_limits(&owner, &[], limits, &mut budget).unwrap();
        let retained = layouts.lease.persistent.get();
        let resident = budget.storage();
        let result = layouts.select_original_closure(&owner, RECURSIVE, &mut budget);
        if cut == 0 {
            result.unwrap();
            assert_eq!(layouts.physical.borrow().rows.len(), 2);
            assert_eq!(layouts.physical.borrow().edges, 2);
        } else {
            assert!(result.is_err());
            assert!(layouts.physical.borrow().rows.is_empty());
            assert!(layouts.physical.borrow().original_extensions.is_empty());
            assert_eq!(layouts.lease.persistent.get(), retained);
            assert_eq!(budget.storage(), resident);
        }
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 293);
    }
}

#[test]
fn original_graph_rejects_hostile_edges_and_missing_reserved_dependencies_without_recursion() {
    for case in 0..4 {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(307).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let mut graph = SourceStorageOriginalGraphV29::default();
        graph.discover(&layouts, RowKey::ty(RECURSIVE), &mut budget).unwrap();
        graph.containment(&layouts, &mut budget).unwrap();
        graph.components(&layouts, &mut budget).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match case {
            0 => {
                graph.edges.iter_mut().find(|row| !row.contained).unwrap().contained = true;
                graph.containment(&layouts, &mut budget)
            }
            1 => {
                graph.edges[0].target = usize::MAX;
                graph.components(&layouts, &mut budget).map(|_| ())
            }
            2 => {
                graph.nodes[0].edges = 0..usize::MAX;
                graph.containment(&layouts, &mut budget)
            }
            _ => layouts.lower_row_resolved(RowKey::ty(RECURSIVE),
                &SourceStorageOriginalResolverV29::Constructing(&graph), &mut budget).map(|_| ()),
        }));
        assert!(result.is_ok());
        assert!(result.unwrap().is_err());
        assert!(layouts.physical.borrow().rows.is_empty());
        drop(graph);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 307);
    }
}

#[test]
fn original_closure_rejects_foreign_owners_guards_and_changed_cache_rows() {
    for case in 0..6 {
        let owner = owner();
        let other = owner_with(types());
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(311).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let schema = layouts.select_original_closure(&owner, RAW, &mut budget).unwrap();
        let result = if case == 0 {
            layouts.select_original_closure(&other, RAW, &mut budget)
        } else if case == 1 {
            let _held = layouts.physical.borrow();
            layouts.select_original_closure(&owner, RECURSIVE, &mut budget)
        } else if case == 2 {
            let _held = layouts.physical.borrow_mut();
            layouts.select_original_closure(&owner, RAW, &mut budget)
        } else {
            let mut physical = layouts.physical.borrow_mut();
            if case == 3 {
                physical.original_extensions.insert(RowKey::ty(RAW), StorageLayoutIdV1(u32::MAX));
            } else {
                let StorageLayoutKindV1::Pointer(pointer) = &mut physical.rows[schema.0 as usize].kind else { panic!("source pointer"); };
                if case == 4 { pointer.stored_bits = 32; }
                else { pointer.access = AccessMode::ReadWrite; }
            }
            drop(physical);
            layouts.select_original_closure(&owner, RAW, &mut budget)
        };
        assert!(result.is_err());
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 311);
    }
}

#[test]
fn original_leaf_selection_has_independent_exact_work_for_original_and_selected_reuse() {
    const WORK: usize = 20_000_000;
    const LIMIT: usize = 64 * 1024 * 1024;
    for original in [false, true] {
        // Owner1 + shape1 + original-owner1 + original lookup32.
        // Original: checked owner1/key1. Selected: lower1/hash1,
        // empty original row search0 + one-key lookup32 + equality1.
        let exact = 1 + 1 + 1 + 32 + if original { 1 + 1 } else { 1 + 1 + 32 + 1 };
        for short in [false, true] {
            let owner = owner();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(197).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, if original { &[WORD] } else { &[] }, &mut budget).unwrap();
            let schema = layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap();
            let rows = layouts.physical.borrow().rows.len();
            let persistent = layouts.lease.persistent.get();
            let pressure = LIMIT - budget.storage();
            budget.reserve_storage(pressure).unwrap();
            assert!(budget.charge_work(usize::MAX).is_err());
            budget.charge_work(WORK - budget.work() - exact + usize::from(short)).unwrap();
            let before = budget.work();
            let result = layouts.select_original_leaf_schema(&owner, WORD, &mut budget);
            if short {
                assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Work(_)));
            } else {
                assert_eq!(result.unwrap(), schema);
            }
            assert_eq!(budget.work(), before + exact - usize::from(short));
            assert_eq!(budget.storage(), LIMIT);
            assert_eq!(layouts.physical.borrow().rows.len(), rows);
            assert_eq!(layouts.lease.persistent.get(), persistent);
            assert_eq!(layouts.release(&mut budget).is_err(), short);
            assert_eq!(budget.storage(), 197 + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), 197);
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }
}

#[test]
fn first_selected_leaf_has_independent_exact_and_one_short_persistent_storage() {
    const LIMIT: usize = 64 * 1024 * 1024;
    // Empty vectors grow to four slots; the empty identity tree is charged
    // two split-path levels of 32 complete key/value/link records.
    let growth = 4 * (size_of::<StorageLayoutV1>() + size_of::<StorageLayoutIdV1>()
        + size_of::<RowKey>() + size_of::<usize>())
        + 2 * 32 * size_of::<((RowKey, u64), Vec<StorageLayoutIdV1>, usize)>();
    for short in [false, true] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(199).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let resident = budget.storage();
        let persistent = layouts.lease.persistent.get();
        let pressure = LIMIT - resident - growth + usize::from(short);
        budget.reserve_storage(pressure).unwrap();
        let result = layouts.select_original_leaf_schema(&owner, WORD, &mut budget);
        if short {
            let first = resource(result.unwrap_err());
            assert!(matches!(first, ArgumentResourceV1::Storage(_)));
            assert_eq!(resource(layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap_err()), first);
            assert_eq!(layouts.physical.borrow().rows.len(), 1);
            assert_eq!(layouts.physical.borrow().selected_keys.len(), 1);
            assert!(layouts.physical.borrow().depths.is_empty());
            assert_eq!(budget.storage(), resident + pressure + growth - 4 * size_of::<usize>());
        } else {
            let schema = result.unwrap();
            assert_eq!(budget.storage(), LIMIT);
            assert_eq!(layouts.lease.persistent.get() - persistent, growth);
            assert_eq!(layouts.physical.borrow().depths, [1]);
            assert_eq!(layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap(), schema);
            assert_eq!(budget.storage(), LIMIT);
        }
        assert!(layouts.keys.is_empty());
        assert_eq!(layouts.release(&mut budget).is_err(), short);
        assert_eq!(budget.storage(), 199 + pressure);
        budget.release_storage(pressure).unwrap();
        assert_eq!(budget.storage(), 199);
    }
}

#[test]
fn selected_original_leaf_rejects_a_foreign_owner_or_active_builder_without_panic() {
    for foreign in [false, true] {
        let owner = owner();
        let other = owner_with(types());
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(211).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
        let before = budget.storage();
        if foreign {
            assert!(layouts.select_original_leaf_schema(&other, WORD, &mut budget).is_err());
        } else {
            let held = layouts.physical.borrow_mut();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
                layouts.select_original_leaf_schema(&owner, WORD, &mut budget)));
            assert!(result.is_ok());
            assert!(result.unwrap().is_err());
            drop(held);
        }
        assert_eq!(budget.storage(), before);
        assert!(layouts.physical.borrow().rows.is_empty());
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 211);
    }
}

#[test]
fn selected_descriptor_auxiliary_leaf_reuse_has_independent_exact_work_and_no_scratch() {
    const WORK: usize = 20_000_000;
    const LIMIT: usize = 64 * 1024 * 1024;
    for original_word in [false, true] {
        // One auxiliary visit + lower-row1/hash1/equality1; the original
        // binary search is paid twice (one key or zero keys). The identity
        // tree has three selected rows, or four when WORD is selected too.
        let exact = 1 + 1 + 1 + 1 + if original_word { 2 + 3 * 16 } else { 4 * 16 };
        for short in [false, true] {
            let owner = owner();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(227).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, if original_word { &[WORD] } else { &[] }, &mut budget).unwrap();
            let element = layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap();
            let schema = layouts.select_schema(&owner, DESCRIPTOR, SourceStorageSelectionV29::Slice {
                element, value_space: AddressSpace::Global, access: AccessMode::ReadOnly,
            }, &mut budget).unwrap();
            let StorageLayoutKindV1::Slice { length, .. } = layouts.physical.borrow().rows[schema.0 as usize].kind
            else { panic!("actual selected descriptor"); };
            let credit = layouts.lease.persistent.get();
            let count = layouts.physical.borrow().rows.len();
            let pressure = LIMIT - budget.storage();
            budget.reserve_storage(pressure).unwrap();
            assert!(budget.charge_work(usize::MAX).is_err());
            budget.charge_work(WORK - budget.work() - exact + usize::from(short)).unwrap();
            let before = budget.work();
            let active = layouts.schema_allocation(&budget).unwrap();
            let result = layouts.selected_descriptor_length_row(DESCRIPTOR, &mut budget);
            drop(active);
            if short {
                assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Work(_)));
            } else { assert_eq!(result.unwrap(), length.layout); }
            assert_eq!(budget.work(), before + exact - usize::from(short));
            assert_eq!(budget.storage(), LIMIT);
            assert_eq!(layouts.lease.persistent.get(), credit);
            assert_eq!(layouts.physical.borrow().rows.len(), count);
            assert_eq!(layouts.release(&mut budget).is_err(), short);
            assert_eq!(budget.storage(), 227 + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), 227);
            drop(budget);
            assert_eq!(work.failed_work(), Some(usize::MAX));
        }
    }
}

#[test]
fn selected_only_descriptor_policies_charge_auxiliary_rows_and_keep_partial_growth_until_release() {
    use fe2o3_kernel_ir::StorageLayoutLimitsV1;
    let exact = StorageLayoutLimitsV1 { rows: 4, edges: 4, containment_depth: 2, object_bytes: 16 };
    for (limits, accepted) in [
        (exact, true), (StorageLayoutLimitsV1 { rows: 3, ..exact }, false),
        (StorageLayoutLimitsV1 { edges: 3, ..exact }, false),
        (StorageLayoutLimitsV1 { containment_depth: 1, ..exact }, false),
        (StorageLayoutLimitsV1 { object_bytes: 15, ..exact }, false),
    ] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(229).unwrap();
        let layouts = SourceStorageLayoutsV29::new_with_limits(&owner, &[], limits, &mut budget).unwrap();
        let element = layouts.select_original_leaf_schema(&owner, WORD, &mut budget).unwrap();
        let before = layouts.lease.persistent.get();
        let result = layouts.select_schema(&owner, DESCRIPTOR, SourceStorageSelectionV29::Slice {
            element, value_space: AddressSpace::Global, access: AccessMode::ReadOnly,
        }, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(layouts.physical.borrow().rows.len(), if accepted { 4 } else { 3 });
        assert_eq!(layouts.physical.borrow().selected_keys.len(), if accepted { 4 } else { 3 });
        assert!(layouts.lease.persistent.get() > before);
        assert!(layouts.keys.is_empty());
        assert_eq!(layouts.release(&mut budget).is_err(), !accepted);
        assert_eq!(budget.storage(), 229);
    }
}

#[test]
fn selected_niche_walk_has_independent_exact_and_one_short_work_without_scratch_allocation() {
    const WORK: usize = 20_000_000;
    const LIMIT: usize = 64 * 1024 * 1024;
    const FLOOR: usize = 163;
    for slice in [false, true] {
        for nested in [false, true] {
            // Loop1; payload identity1/descent1; field step1/prefix2/
            // child identity1/descent1; selected pointer5/pointee identity1.
            // Nested array+tuple add (step1+identity1+descent1)*2+prefix1.
            // A slice adds data identity1 and one containment descent1.
            let exact = 1 + 1 + 1 + 1 + 2 + 1 + 1 + 5 + 1
                + usize::from(nested) * (3 * 2 + 1) + usize::from(slice) * 2;
            for short in [false, true] {
                let fixture = selected_niche_fixture_v29(slice, nested);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(FLOOR).unwrap();
                let layouts = SourceStorageLayoutsV29::new(&fixture.owner, &[WORD], &mut budget).unwrap();
                let selected = super::representation_tests::selected_niche_rows_v29(
                    &fixture, &layouts, AddressSpace::Global, &mut budget);
                let resident = budget.storage();
                let persistent = layouts.lease.persistent.get();
                let rows = layouts.physical.borrow().rows.len();
                let pressure = LIMIT - resident;
                budget.reserve_storage(pressure).unwrap();
                assert!(matches!(budget.charge_work(usize::MAX), Err(ArgumentResourceV1::Work(_))));
                budget.charge_work(WORK - budget.work() - exact + usize::from(short)).unwrap();
                let before = budget.work();
                let result = layouts.selected_niche_pointer_row(fixture.enumeration, &selected.variants, &mut budget);
                if short {
                    let first = resource(result.unwrap_err());
                    assert!(matches!(first, ArgumentResourceV1::Work(_)));
                    assert_eq!(resource(layouts.lease.reserve(usize::MAX, &mut budget).unwrap_err()), first);
                    assert_eq!(budget.work(), before + exact - 1);
                } else {
                    let row = result.unwrap();
                    assert!(matches!(row.kind, StorageLayoutKindV1::Pointer(pointer)
                        if pointer.pointee == selected.mixed && pointer.value_space == AddressSpace::Global
                            && pointer.encoded_space == AddressSpace::Generic && pointer.stored_bits == 64));
                    assert_eq!(budget.work(), before + exact);
                }
                assert_eq!(budget.storage(), LIMIT);
                assert_eq!(layouts.lease.persistent.get(), persistent);
                assert_eq!(layouts.physical.borrow().rows.len(), rows);
                let released = layouts.release(&mut budget);
                assert_eq!(released.is_err(), short);
                assert_eq!(budget.storage(), FLOOR + pressure);
                budget.release_storage(pressure).unwrap();
                assert_eq!(budget.storage(), FLOOR);
                drop(budget);
                assert_eq!(work.failed_work(), Some(usize::MAX));
            }
        }
    }
}

#[test]
fn selected_niche_containment_depth_and_shared_component_meter_have_exact_boundaries() {
    for slice in [false, true] {
        for short in [false, true] {
            let fixture = selected_niche_fixture_v29(slice, true);
            let mut work = work();
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(167).unwrap();
            let mut layouts = SourceStorageLayoutsV29::new(&fixture.owner, &[WORD], &mut budget).unwrap();
            let selected = super::representation_tests::selected_niche_rows_v29(
                &fixture, &layouts, AddressSpace::Private, &mut budget);
            // Enum -> payload -> array -> tuple -> pointer, plus descriptor data.
            layouts.limits.containment_depth = 5 + usize::from(slice) - usize::from(short);
            let result = layouts.selected_niche_pointer_row(fixture.enumeration, &selected.variants, &mut budget);
            assert_eq!(result.is_err(), short);
            let resident = budget.storage();
            let mut components = 0;
            for _ in 0..MAX_SSA_VALUE_COMPONENTS_V1 {
                layouts.selected_niche_work(&mut components, &mut budget).unwrap();
            }
            assert_eq!(components, MAX_SSA_VALUE_COMPONENTS_V1);
            assert!(layouts.selected_niche_work(&mut components, &mut budget).is_err());
            assert_eq!(components, MAX_SSA_VALUE_COMPONENTS_V1 + 1);
            assert_eq!(budget.storage(), resident);
            layouts.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 167);
        }
    }
}

#[test]
fn selected_niche_schema_growth_keeps_live_credit_when_identity_publication_runs_out_of_storage() {
    const LIMIT: usize = 64 * 1024 * 1024;
    for deny in [false, true] {
        let fixture = selected_niche_fixture_v29(true, true);
        let mut work = work();
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(173).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&fixture.owner, &[WORD], &mut budget).unwrap();
        let selected = super::representation_tests::selected_niche_rows_v29(
            &fixture, &layouts, AddressSpace::Private, &mut budget);
        let count = layouts.physical.borrow().rows.len();
        assert!(layouts.physical.borrow().rows.capacity() > count);
        let identities = layouts.physical.borrow().selected_keys.len();
        let old = layouts.lease.persistent.get();
        let pressure = if deny { LIMIT - budget.storage() } else { 0 };
        budget.reserve_storage(pressure).unwrap();
        let result = layouts.select_schema(&fixture.owner, fixture.enumeration,
            SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget);
        if deny {
            assert!(matches!(resource(result.unwrap_err()), ArgumentResourceV1::Storage(_)));
            // The prepaid row slot can be filled before the new map identity
            // is denied. It remains covered by live table credit until release.
            assert_eq!(layouts.physical.borrow().rows.len(), count + 1);
            assert_eq!(layouts.physical.borrow().selected_keys.len(), identities);
            assert_eq!(layouts.lease.persistent.get(), old);
            assert!(layouts.check_selected_schema(&fixture.owner, fixture.enumeration,
                StorageLayoutIdV1(count as u32), &mut budget).is_err());
        } else {
            let schema = result.unwrap();
            assert_eq!(layouts.physical.borrow().rows.len(), count + 2);
            assert!(layouts.lease.persistent.get() > old);
            let persistent = layouts.lease.persistent.get();
            for _ in 0..3 {
                assert_eq!(layouts.select_schema(&fixture.owner, fixture.enumeration,
                    SourceStorageSelectionV29::Enum { variants: &selected.variants }, &mut budget).unwrap(), schema);
                assert_eq!(layouts.lease.persistent.get(), persistent);
                assert_eq!(layouts.physical.borrow().rows.len(), count + 2);
            }
        }
        assert_eq!(layouts.release(&mut budget).is_err(), deny);
        assert_eq!(budget.storage(), 173 + pressure);
        budget.release_storage(pressure).unwrap();
        assert_eq!(budget.storage(), 173);
    }
}

fn resource(error: Error) -> ArgumentResourceV1 {
    match error {
        Error::ArgumentCorrespondenceResource(resource) => resource,
        other => panic!("not a resource denial: {other:?}"),
    }
}

// Independent constructor schedule: one entry work unit, two emission_vec
// admissions at three units each, and one initialization visit per source type.
// Empty demands do not sort, follow, lower, compare or push any row.
fn empty_table_work(type_count: usize) -> usize {
    1 + 3 + type_count + 3
}
fn table_header() -> usize {
    size_of::<SourceStorageLayoutsV29<'_>>()
        + size_of::<Result<SourceStorageLayoutsV29<'_>, Error>>()
        + size_of::<Vec<bool>>()
        + size_of::<Vec<StorageFieldV1>>()
        + size_of::<Vec<StorageVariantV1>>()
        + size_of::<Vec<StorageLayoutV1>>()
        + size_of::<Result<StorageLayoutV1, Error>>()
        + 6 * size_of::<usize>()
}

#[test]
fn empty_source_bound_table_has_independently_derived_exact_work_and_peak_storage() {
    let owner = owner();
    let type_count = owner.source_semantic().types().len();
    let work_limit = empty_table_work(type_count);
    let storage_limit = table_header() + type_count * size_of::<bool>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let table = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    assert_eq!(budget.work(), work_limit);
    assert_eq!(budget.storage(), table_header());
    assert_eq!(budget.peak_storage(), storage_limit);
    table.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit - 1);
    let mut budget = Budget::new(&mut work, storage_limit);
    let error = match SourceStorageLayoutsV29::new(&owner, &[], &mut budget) {
        Ok(_) => panic!("short work admitted"),
        Err(error) => error,
    };
    assert!(matches!(resource(error), ArgumentResourceV1::Work(_)));
    assert_eq!(budget.storage(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit - 1);
    let error = match SourceStorageLayoutsV29::new(&owner, &[], &mut budget) {
        Ok(_) => panic!("short storage admitted"),
        Err(error) => error,
    };
    assert!(matches!(resource(error), ArgumentResourceV1::Storage(_)));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn vector_and_push_admission_happens_before_allocation_or_mutation() {
    const COUNT: usize = 2;
    const WORK: usize = 3 + 2 * COUNT;
    const STORAGE: usize = COUNT * size_of::<u64>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let lease = Lease::new(&budget);
    let mut values = lease.vector(COUNT, &mut budget).unwrap();
    lease.push(&mut values, 17_u64, &mut budget).unwrap();
    lease.push(&mut values, 23_u64, &mut budget).unwrap();
    assert_eq!(values, [17, 23]);
    assert_eq!(budget.work(), WORK);
    assert_eq!(budget.storage(), STORAGE);
    lease.discard_vec(values, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(lease.owned.get(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK - 1);
    let mut budget = Budget::new(&mut work, STORAGE);
    let lease = Lease::new(&budget);
    let mut values = lease.vector(COUNT, &mut budget).unwrap();
    lease.push(&mut values, 17_u64, &mut budget).unwrap();
    let first = resource(lease.push(&mut values, 23_u64, &mut budget).unwrap_err());
    assert!(matches!(first, ArgumentResourceV1::Work(_)));
    assert_eq!(values, [17]);
    assert_eq!(
        resource(lease.reserve(usize::MAX, &mut budget).unwrap_err()),
        first
    );
    lease.discard_vec(values, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn constructor_cleanup_preserves_nonzero_caller_floor_and_unrelated_extra_credit() {
    let owner = owner();
    const FLOOR: usize = 73;
    const EXTRA: usize = 91;
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(FLOOR).unwrap();
    let table = SourceStorageLayoutsV29::new(&owner, &[PAIR], &mut budget).unwrap();
    budget.reserve_storage(EXTRA).unwrap();
    table.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + EXTRA);
    let error = match SourceStorageLayoutsV29::new(&owner, &[SLICE], &mut budget) {
        Ok(_) => panic!("unsized storage admitted"),
        Err(error) => error,
    };
    assert!(!matches!(error, Error::ArgumentCorrespondenceResource(_)));
    assert_eq!(budget.storage(), FLOOR + EXTRA);
}

#[test]
fn foreign_work_ledger_and_lost_storage_custody_cannot_refund_caller_credit() {
    let mut work = work();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 1000);
    let mut foreign = Budget::new(&mut foreign_work, 1000);
    budget.reserve_storage(17).unwrap();
    foreign.reserve_storage(17).unwrap();
    let lease = Lease::new(&budget);
    lease.reserve(23, &mut budget).unwrap();
    assert_eq!(
        resource(lease.refund(23, &mut foreign).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!(foreign.storage(), 17);
    assert_eq!(budget.storage(), 40);
    lease.refund(23, &mut budget).unwrap();
    assert_eq!(budget.storage(), 17);
    assert_eq!(
        resource(lease.work(0, &mut budget).unwrap_err()),
        ArgumentResourceV1::Accounting
    );

    let lease = Lease::new(&budget);
    lease.reserve(23, &mut budget).unwrap();
    budget.release_storage(24).unwrap();
    assert_eq!(
        resource(lease.refund(23, &mut budget).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!(budget.storage(), 16);
}

#[test]
fn arithmetic_denial_is_sticky_and_allocator_denial_is_not_misreported_as_semantic_success() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let lease = Lease::new(&budget);
    let first = resource(lease.vector::<u64>(usize::MAX, &mut budget).unwrap_err());
    assert_eq!(first, ArgumentResourceV1::Arithmetic);
    assert_eq!(resource(lease.work(0, &mut budget).unwrap_err()), first);
    assert_eq!(budget.storage(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let lease = Lease::new(&budget);
    // Vec rejects capacities above isize::MAX before attempting an allocation.
    let count = isize::MAX as usize + 1;
    let first = resource(lease.vector::<u8>(count, &mut budget).unwrap_err());
    assert_eq!(first, ArgumentResourceV1::Allocation);
    assert_eq!(resource(lease.reserve(0, &mut budget).unwrap_err()), first);
    lease.refund(lease.owned.get(), &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

// The V1046 admitted fixture owns the original source types and locals. These
// extra declarations stay local to resource tests, without changing that corpus.
const RESOURCE_UNION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(21);
const RESOURCE_WRAPPER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(22);

fn union_resource_owner() -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let mut declarations = types();
    assert_eq!(BYTE_OCTET.index(), 20);
    assert_eq!(declarations.len(), BYTE_OCTET.index() as usize + 1);
    assert_eq!(declarations.len(), RESOURCE_UNION.index() as usize);
    assert_eq!(RESOURCE_WRAPPER.index(), RESOURCE_UNION.index() + 1);
    declarations.push(declaration(
        211,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::Union { field_count: 2 },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![PAIR, WORD]).unwrap()),
    ));
    declarations.push(declaration(
        212,
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![RESOURCE_UNION, WORD]).unwrap(),
        ),
    ));
    owner_with(declarations)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnionResourceOperation {
    ClearDefault,
    FilterOverlap,
}

impl UnionResourceOperation {
    fn work(self) -> usize {
        match self {
            // Place check1; inherited proof (check1, scans2+1+1,
            // empty guards vector3, one path step1); replacement vector3;
            // old facts (prefix1+push2, prefix2+push2, prefix2+push2);
            // place copy(vector3+step1), empty guards3, push2, sorts2+2+1.
            Self::ClearDefault => {
                1 + (1 + 2 + 1 + 1 + 3 + 1)
                    + 3
                    + (1 + 2 + 2 + 2 + 2 + 2)
                    + (3 + 1)
                    + 3
                    + 2
                    + (2 + 2 + 1)
            }
            // Place check1 + replacement vector3. Four facts: enclosing root
            // visit/prefix/push1+1+2; union1+2+2; removed leaf1+3;
            // nonoverlapping leaf1+push2 (no prefix comparison).
            Self::FilterOverlap => 1 + 3 + (1 + 1 + 2) + (1 + 2 + 2) + (1 + 3) + (1 + 2),
        }
    }

    fn prefix_before_vector(self) -> usize {
        match self {
            Self::ClearDefault => 1 + (1 + 2 + 1 + 1 + 3 + 1) + 3,
            Self::FilterOverlap => 1 + 3,
        }
    }

    fn proof_headers(self) -> usize {
        match self {
            Self::ClearDefault => {
                size_of::<InitializationProofV29>()
                    + size_of::<Result<InitializationProofV29, Error>>()
            }
            Self::FilterOverlap => 0,
        }
    }

    fn replacement_headers(self) -> usize {
        self.proof_headers() + 2 * size_of::<Vec<InitializationFact<'_, '_>>>()
    }

    fn peak(self) -> usize {
        let fact = size_of::<InitializationFact<'_, '_>>();
        let place = size_of::<SourceStorageSubobjectV29<'_, '_>>();
        let step = size_of::<SubobjectStep>();
        // Both operations copy into four exact fact slots while the old vector
        // is still live. Later default insertion has refunded three old slots.
        assert!(4 * fact >= fact + place + step);
        self.replacement_headers() + 4 * fact
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnionResourceBoundary {
    Exact,
    WorkShort,
    StorageShort,
}

fn union_resource_fact(
    state: &SourceStorageStateV29<'_, '_>,
    index: usize,
    path: &[SubobjectStep],
    initialized: bool,
) {
    let fact = &state.facts[index];
    assert_eq!(fact.place.path, path);
    assert_eq!(fact.initialized, initialized);
    assert!(fact.guards.is_empty());
}

fn union_resource_boundary(
    operation: UnionResourceOperation,
    boundary: UnionResourceBoundary,
    prior_denials: bool,
) {
    const WORK_LIMIT: usize = 20_000_000;
    const STORAGE_LIMIT: usize = 64 * 1024 * 1024;
    const FLOOR: usize = 73;
    const EXTRA: usize = 91;
    let owner = union_resource_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let expected_failed_work = production_call_instances_v1::with_production_call_instances_v1(
        &owner, ROOT, &mut budget,
        |instances, budget| -> Result<_, production_call_instances_v1::ProductionCallInstanceErrorV1> {
            let entry_floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(instances.owner(), &[RESOURCE_WRAPPER], budget).unwrap();
            let root = layouts.root_subobject(instances.owner(), RESOURCE_WRAPPER, budget).unwrap();
            let union = layouts.project_step(&root, SubobjectStep::Field(0), budget).unwrap();
            let alias = layouts.project_step(&union, SubobjectStep::Field(0), budget).unwrap();
            let low = layouts.project_step(&alias, SubobjectStep::Field(0), budget).unwrap();
            let high = layouts.project_step(&alias, SubobjectStep::Field(1), budget).unwrap();
            let written = layouts.project_step(&union, SubobjectStep::Field(1), budget).unwrap();
            let mut state = SourceStorageStateV29::new(
                &layouts, instances, instances.root(), local_for(RESOURCE_WRAPPER), budget,
            ).unwrap();
            assert_eq!((root.range.start, root.range.end), (0, 24));
            assert_eq!((union.range.start, union.range.end), (0, 16));
            assert_eq!((low.range.start, low.range.end), (0, 8));
            assert_eq!((high.range.start, high.range.end), (8, 9));
            assert_eq!(written.range, low.range);
            assert_eq!((union.initialization_floor, alias.initialization_floor), (0, 2));
            assert!(std::ptr::eq(state.instances, instances));
            assert_eq!(state.instance, instances.root());
            assert_eq!(state.local, local_for(RESOURCE_WRAPPER));
            // Seed the private transfer helper's explicit typed facts through
            // production setters; no synthetic path, row or owner is fabricated.
            state.initialize(&root, budget).unwrap();
            state.set_fact(&low, true, budget).unwrap();
            state.set_fact(&high, true, budget).unwrap();
            let nested = [SubobjectStep::Field(0)];
            let low_path = [SubobjectStep::Field(0), SubobjectStep::Field(0), SubobjectStep::Field(0)];
            let high_path = [SubobjectStep::Field(0), SubobjectStep::Field(0), SubobjectStep::Field(1)];
            assert_eq!((state.facts.len(), state.facts.capacity()), (3, 3));
            union_resource_fact(&state, 0, &[], true);
            union_resource_fact(&state, 1, &low_path, true);
            union_resource_fact(&state, 2, &high_path, true);
            assert_eq!(state.facts[1].place.path.capacity(), 3);
            assert_eq!(state.facts[2].place.path.capacity(), 3);
            if operation == UnionResourceOperation::FilterOverlap {
                state.clear_prefix_default(&union, budget).unwrap();
                assert_eq!((state.facts.len(), state.facts.capacity()), (4, 4));
                union_resource_fact(&state, 1, &nested, false);
            }
            budget.reserve_storage(EXTRA).unwrap();

            // Same-ledger pressure isolates independently derived helper costs.
            // No successful measured total or changed production quota is used.
            let allowed_work = operation.work() - usize::from(boundary == UnionResourceBoundary::WorkShort);
            let allowed_storage = operation.peak() - usize::from(boundary == UnionResourceBoundary::StorageShort);
            budget.charge_work(WORK_LIMIT - budget.work() - allowed_work).unwrap();
            let pressure = STORAGE_LIMIT - budget.storage() - allowed_storage;
            budget.reserve_storage(pressure).unwrap();
            let start_work = budget.work();
            let start_storage = budget.storage();
            assert_eq!(budget.peak_storage(), start_storage);
            assert_eq!(layouts.lease.failure.resource(), None);
            if prior_denials {
                assert!(budget.charge_work(allowed_work + 33).is_err());
                assert!(budget.reserve_storage(allowed_storage + 29).is_err());
                assert_eq!(layouts.lease.failure.resource(), None);
            }

            let outcome = match operation {
                UnionResourceOperation::ClearDefault => state.clear_prefix_default(&union, budget),
                UnionResourceOperation::FilterOverlap => state.invalidate_union_field_overlaps(&alias, written.range, budget),
            };
            let first = outcome.err().map(resource);
            let fact = size_of::<InitializationFact<'_, '_>>();
            let place = size_of::<SourceStorageSubobjectV29<'_, '_>>();
            let step = size_of::<SubobjectStep>();
            let (accepted, final_storage, peak_storage) = match boundary {
                UnionResourceBoundary::Exact => {
                    assert_eq!(first, None);
                    union_resource_fact(&state, 0, &[], true);
                    union_resource_fact(&state, 1, &nested, false);
                    let final_storage = match operation {
                        UnionResourceOperation::ClearDefault => {
                            assert_eq!((state.facts.len(), state.facts.capacity()), (4, 4));
                            union_resource_fact(&state, 2, &low_path, true);
                            union_resource_fact(&state, 3, &high_path, true);
                            start_storage + fact + place + step
                        }
                        UnionResourceOperation::FilterOverlap => {
                            assert_eq!((state.facts.len(), state.facts.capacity()), (3, 4));
                            union_resource_fact(&state, 2, &high_path, true);
                            start_storage - place - 3 * step
                        }
                    };
                    (operation.work(), final_storage, start_storage + operation.peak())
                }
                UnionResourceBoundary::WorkShort => {
                    assert!(matches!(first, Some(ArgumentResourceV1::Work(error)) if error.actual() == start_work + operation.work() && error.limit() == WORK_LIMIT));
                    assert!(state.facts.is_empty());
                    let (last_charge, final_storage) = match operation {
                        // Final root/union ordering comparison is one unit.
                        UnionResourceOperation::ClearDefault => (1, start_storage + operation.replacement_headers() + fact + place + step),
                        // Last preserved sibling push is an atomic two-unit charge.
                        UnionResourceOperation::FilterOverlap => (2, start_storage + operation.replacement_headers() + 4 * fact - place - 3 * step),
                    };
                    (operation.work() - last_charge, final_storage, start_storage + operation.peak())
                }
                UnionResourceBoundary::StorageShort => {
                    assert!(matches!(first, Some(ArgumentResourceV1::Storage(error)) if error.actual() == start_storage + operation.peak() && error.limit() == STORAGE_LIMIT));
                    let original_count = if operation == UnionResourceOperation::ClearDefault { 3 } else { 4 };
                    assert_eq!((state.facts.len(), state.facts.capacity()), (original_count, original_count));
                    let kept = start_storage + operation.replacement_headers();
                    (operation.prefix_before_vector(), kept, kept)
                }
            };
            assert_eq!(budget.work(), start_work + accepted);
            assert_eq!(budget.storage(), final_storage);
            assert_eq!(budget.peak_storage(), peak_storage);
            assert_eq!(layouts.lease.failure.resource(), first);
            let failed_work = if prior_denials { Some(WORK_LIMIT + 33) }
                else if boundary == UnionResourceBoundary::WorkShort { Some(start_work + operation.work()) }
                else { None };
            let failed_storage = if prior_denials { Some(STORAGE_LIMIT + 29) }
                else if boundary == UnionResourceBoundary::StorageShort { Some(start_storage + operation.peak()) }
                else { None };
            assert_eq!(budget.failed_storage(), failed_storage);
            if let Some(first) = first {
                // The lease remembers its own failure, independently of any
                // earlier global denial; further source queries fail closed.
                assert_eq!(resource(layouts.lease.work(0, budget).unwrap_err()), first);
                assert_eq!(resource(layouts.lease.reserve(usize::MAX, budget).unwrap_err()), first);
                assert_eq!(resource(state.is_initialized(&root, budget).unwrap_err()), first);
                assert_eq!((budget.work(), budget.storage(), budget.peak_storage()), (start_work + accepted, final_storage, peak_storage));
                assert_eq!(budget.failed_storage(), failed_storage);
            }
            // All actual dependent values drop before the shared lease refunds
            // successful backing and already-dropped error-path temporaries.
            drop(state);
            drop((root, union, alias, low, high, written));
            let released = layouts.release(budget).err().map(resource);
            assert_eq!(released, first);
            assert_eq!(budget.storage(), entry_floor + EXTRA + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), entry_floor + EXTRA);
            budget.release_storage(EXTRA).unwrap();
            assert_eq!(budget.storage(), entry_floor);
            assert_eq!(budget.peak_storage(), peak_storage);
            assert_eq!(budget.failed_storage(), failed_storage);
            Ok(failed_work)
        },
    ).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    drop(budget);
    assert_eq!(work.failed_work(), expected_failed_work);
}

#[test]
fn union_default_replacement_has_exact_and_one_short_independent_resources() {
    assert_eq!(UnionResourceOperation::ClearDefault.work(), 38);
    assert_eq!(
        UnionResourceOperation::ClearDefault.prefix_before_vector(),
        13
    );
    for boundary in [
        UnionResourceBoundary::Exact,
        UnionResourceBoundary::WorkShort,
        UnionResourceBoundary::StorageShort,
    ] {
        union_resource_boundary(UnionResourceOperation::ClearDefault, boundary, false);
    }
}

#[test]
fn union_overlap_vector_replacement_has_exact_and_one_short_independent_resources() {
    assert_eq!(UnionResourceOperation::FilterOverlap.work(), 20);
    assert_eq!(
        UnionResourceOperation::FilterOverlap.prefix_before_vector(),
        4
    );
    for boundary in [
        UnionResourceBoundary::Exact,
        UnionResourceBoundary::WorkShort,
        UnionResourceBoundary::StorageShort,
    ] {
        union_resource_boundary(UnionResourceOperation::FilterOverlap, boundary, false);
    }
}

#[test]
fn union_replacement_cleanup_keeps_prior_global_denials_separate_from_sticky_lease_failure() {
    for operation in [
        UnionResourceOperation::ClearDefault,
        UnionResourceOperation::FilterOverlap,
    ] {
        for boundary in [
            UnionResourceBoundary::Exact,
            UnionResourceBoundary::WorkShort,
            UnionResourceBoundary::StorageShort,
        ] {
            union_resource_boundary(operation, boundary, true);
        }
    }
}
