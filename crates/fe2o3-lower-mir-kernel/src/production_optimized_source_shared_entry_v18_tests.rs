fn shared_entry_mapped_roots_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    shared_entry_growing_roots_owner_v18(0)
}

fn shared_entry_growing_roots_owner_v18(extra: usize) -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase::READ);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let mut arguments = vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        UNIT,
        SemanticAbiPassModeV1::Ignore,
    ))];
    arguments.extend_from_slice(original.abi().arguments());
    for _ in 0..extra {
        arguments.push(original.abi().arguments()[0].clone());
    }
    let mut ownership = vec![SemanticSourceArgumentOwnershipV1::ByValue];
    ownership.extend_from_slice(original.abi().source_argument_ownership());
    ownership.extend(std::iter::repeat_n(
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        extra,
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([227; 32]),
        original.abi().layout_identity(),
        original.abi().canon_abi(),
        original.abi().extern_abi(),
        original.abi().can_unwind(),
        original.abi().c_variadic(),
        original.abi().fixed_count() + 1 + u32::try_from(extra).unwrap(),
        arguments,
        original.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    let mut locals: Vec<_> = original
        .locals()
        .iter()
        .map(|decl| {
            let role = match decl.role() {
                SemanticLocalRoleV1::Argument(ordinal) => {
                    SemanticLocalRoleV1::Argument(ordinal + 1)
                }
                role => role,
            };
            SemanticLocalDeclV1::new(decl.identity(), decl.ty(), role, decl.source())
        })
        .collect();
    locals.push(local(228, UNIT, SemanticLocalRoleV1::Argument(0)));
    for index in 0..extra {
        locals.push(local(
            240 + u8::try_from(index).unwrap(),
            original
                .locals()
                .iter()
                .find(|decl| decl.role() == SemanticLocalRoleV1::Argument(0))
                .unwrap()
                .ty(),
            SemanticLocalRoleV1::Argument(original.abi().fixed_count() + 1 + index as u32),
        ));
    }
    let first = function(
        229,
        SemanticFunctionRoleV1::KernelRoot,
        abi.clone(),
        locals.clone(),
        original.blocks().to_vec(),
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"z_shared_entry_first".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([229; 32]),
        original.kernel_entry().unwrap().source_contract(),
    ));
    let second = function(
        230,
        SemanticFunctionRoleV1::KernelRoot,
        abi,
        locals,
        original.blocks().to_vec(),
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"a_shared_entry_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([230; 32]),
        original.kernel_entry().unwrap().source_contract(),
    ));
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        vec![first, second],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run_shared_entry_fixture_v18(
    mapped: bool,
    root: usize,
    mode: slice_view_v1::SharedEntryTestV18,
    work: usize,
    storage: usize,
    observed: &std::cell::Cell<[usize; 6]>,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let owner = if mapped {
        shared_entry_mapped_roots_owner_v18()
    } else {
        descriptor_source_owner(DescriptorCase::READ)
    };
    run_descriptor_role_owner_v18(owner, work, storage, |original, optimized, budget| {
        slice_view_v1::test_shared_entry_region_v18(
            original, optimized, root, mode, work, budget, observed,
        )
    })
}

#[test]
fn pending_shared_entry_region_joins_genuine_source_native_and_local_read() {
    let observed = std::cell::Cell::new([0; 6]);
    run_shared_entry_fixture_v18(
        false,
        0,
        slice_view_v1::SharedEntryTestV18::Positive,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        &observed,
    )
    .0
    .unwrap();
    assert_eq!(observed.get()[0], 3);
    assert_eq!(&observed.get()[1..4], &[0, 0, 0]);
}

#[test]
fn pending_shared_entry_region_keeps_root_and_logical_physical_ordinals_distinct() {
    for root in 0..2 {
        let observed = std::cell::Cell::new([0; 6]);
        run_shared_entry_fixture_v18(
            true,
            root,
            slice_view_v1::SharedEntryTestV18::Positive,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0
        .unwrap();
        let values = observed.get();
        assert_eq!(&values[..4], &[3, root, 1, 0]);
        assert_eq!(values[5], root, "exact original semantic root");
    }
}

#[test]
fn pending_shared_entry_region_preserves_complete_exact_and_one_short_limits() {
    let run = |work, storage| {
        let observed = std::cell::Cell::new([0; 6]);
        let result = run_shared_entry_fixture_v18(
            true,
            1,
            slice_view_v1::SharedEntryTestV18::Positive,
            work,
            storage,
            &observed,
        );
        (result, observed.get())
    };
    let ((result, work, peak), observed) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert_eq!(observed[0], 3);
    let ((result, exact_work, exact_peak), exact_observed) = run(work, peak);
    result.unwrap();
    assert_eq!(
        (exact_work, exact_peak, exact_observed),
        (work, peak, observed)
    );
    let ((result, accepted, _), _) = run(work - 1, peak);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Work(error)
        if error.actual() == work && error.limit() == work - 1),
        "{resource:?}"
    );
    assert!(accepted <= work - 1);
    let ((result, _, accepted_peak), _) = run(work, peak - 1);
    let resource = source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err());
    assert!(
        matches!(resource, ArgumentResourceV1::Storage(error)
        if error.actual() == peak && error.limit() == peak - 1),
        "{resource:?}"
    );
    assert!(accepted_peak <= peak - 1);
}

#[test]
fn pending_shared_entry_region_reuses_root_scope_without_per_read_growth() {
    let mut previous: Option<[usize; 4]> = None;
    for extra in [0, 4, 12] {
        let mut repeated: Option<[usize; 4]> = None;
        for reads in [1, 4, 16] {
            let observed = std::cell::Cell::new([0; 6]);
            run_descriptor_role_owner_v18(
                shared_entry_growing_roots_owner_v18(extra),
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                |original, optimized, budget| {
                    slice_view_v1::test_shared_entry_region_v18(
                        original,
                        optimized,
                        1,
                        slice_view_v1::SharedEntryTestV18::Scaling(reads),
                        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                        budget,
                        &observed,
                    )
                },
            )
            .0
            .unwrap();
            let values = observed.get();
            assert_eq!((values[0], values[4]), (reads, 1));
            assert!(values[3] >= 4 + extra, "complete selected-root walk");
            let metrics = [values[1], values[2], values[3], values[5]];
            if let Some(expected) = repeated {
                assert_eq!(metrics, expected);
            }
            repeated = Some(metrics);
        }
        let metrics = repeated.unwrap();
        if let Some([work, floor, nodes, construction]) = previous {
            assert!(metrics[0] > work && metrics[1] > floor && metrics[2] > nodes);
            assert!(
                metrics[0] <= 4 * work && metrics[3] <= 4 * construction,
                "bounded root construction and two selected-root walks: {metrics:?}"
            );
        }
        previous = Some(metrics);
    }
}

#[test]
fn pending_shared_entry_region_does_not_convert_an_issued_read_contract() {
    let owner = global_native_cross_descriptor_copy_owner_v18();
    let abi = issued_descriptor_role_abi_v18(&owner);
    let observed = std::cell::Cell::new([0; 6]);
    let result = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        |original, optimized, budget| {
            slice_view_v1::test_shared_entry_region_v18(
                original,
                optimized,
                0,
                slice_view_v1::SharedEntryTestV18::IssuedRefusal,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                budget,
                &observed,
            )
        },
    )
    .0;
    assert_eq!(
        observed.get()[0],
        1,
        "actual issued local read must first succeed: {result:?}"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "shared entry region requires original SharedSlice"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn pending_shared_entry_region_rejects_copied_parameter_coverage_and_identity() {
    for fault in 0..6 {
        let observed = std::cell::Cell::new([0; 6]);
        let result = run_shared_entry_fixture_v18(
            true,
            1,
            slice_view_v1::SharedEntryTestV18::NodeFault(fault),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(
            observed.get()[0],
            1,
            "genuine positive before predicate fault {fault}: {result:?}"
        );
        result.unwrap();
    }
}

#[test]
fn pending_shared_entry_region_keeps_semantic_and_physical_root_joins_exact() {
    for fault in 0..2 {
        let observed = std::cell::Cell::new([0; 6]);
        let result = run_shared_entry_fixture_v18(
            true,
            1,
            slice_view_v1::SharedEntryTestV18::RootJoinFault(fault),
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(
            observed.get()[0],
            1,
            "genuine positive before internal root join fault: {result:?}"
        );
        let detail = if fault == 0 {
            "original source argument correspondence"
        } else {
            "shared entry region changed original root"
        };
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(actual)) if actual == detail),
            "{result:?}"
        );
    }
}

#[test]
fn pending_shared_entry_region_rejects_foreign_formal_owner() {
    let observed = std::cell::Cell::new([0; 6]);
    let result = run_shared_entry_fixture_v18(
        false,
        0,
        slice_view_v1::SharedEntryTestV18::ForeignFacts,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        &observed,
    )
    .0;
    assert_eq!(observed.get()[0], 1, "{result:?}");
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global read changed formal owner"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn pending_shared_entry_region_retains_constructor_refusal_before_owned_callback_drop() {
    let observed = std::cell::Cell::new([0; 6]);
    let result = run_shared_entry_fixture_v18(
        false,
        0,
        slice_view_v1::SharedEntryTestV18::RootHeaderDropPanic,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        &observed,
    )
    .0;
    assert_eq!(observed.get()[0], 1, "{result:?}");
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
        ArgumentResourceV1::Storage(error))) if error.limit() == MODULE_LIMIT),
        "{result:?}"
    );
}

#[test]
fn pending_shared_entry_region_preserves_selected_error_panic_and_first_resource() {
    for mode in [
        slice_view_v1::SharedEntryTestV18::SelectedError,
        slice_view_v1::SharedEntryTestV18::Panic,
        slice_view_v1::SharedEntryTestV18::WorkThenDropPanic,
        slice_view_v1::SharedEntryTestV18::SwallowedQueryThenError,
    ] {
        let observed = std::cell::Cell::new([0; 6]);
        let result = run_shared_entry_fixture_v18(
            false,
            0,
            mode,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            &observed,
        )
        .0;
        assert_eq!(observed.get()[0], 1, "mode {mode:?}: {result:?}");
        match mode {
            slice_view_v1::SharedEntryTestV18::SelectedError => assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "shared entry callback sentinel"
                ))
            )),
            slice_view_v1::SharedEntryTestV18::Panic => result.unwrap(),
            _ => assert!(
                matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work(error))) if error.limit() == OPTIMIZED_SOURCE_WORK_LIMIT_V18
                    && error.actual() > error.limit()),
                "{result:?}"
            ),
        }
    }
}

#[test]
fn pending_shared_entry_region_foreign_query_never_debits_or_refunds_the_other_ledger() {
    for extra in [0, 17] {
        let observed = std::cell::Cell::new([0; 6]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_shared_entry_fixture_v18(
                false,
                0,
                slice_view_v1::SharedEntryTestV18::ForeignQuery(extra),
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
                &observed,
            )
        }));
        assert_eq!(
            observed.get()[0],
            1,
            "foreign budget refusal is tested inside the real source scope"
        );
        assert!(
            result.is_err(),
            "the unchanged fixture floor assertion detects deliberately denied refund"
        );
    }
}

#[test]
fn pending_shared_entry_region_callback_floor_loss_growth_and_ledger_substitution_deny_refund() {
    for change in 0..4 {
        for disposition in 0..3 {
            let observed = std::cell::Cell::new([0; 6]);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_shared_entry_fixture_v18(
                    false,
                    0,
                    slice_view_v1::SharedEntryTestV18::Custody {
                        change,
                        disposition,
                    },
                    OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                    MODULE_LIMIT,
                    &observed,
                )
            }));
            assert_eq!(
                observed.get()[0],
                1,
                "actual callback must run for {change}/{disposition}"
            );
            assert!(observed.get()[1] > MODULE_FLOOR);
            assert!(
                result.is_err(),
                "unchanged fixture must observe denied whole-scope refund"
            );
        }
    }
}
