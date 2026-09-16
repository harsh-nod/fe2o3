fn borrowed_n_ranked_fixture_v1(
    exports: &[&str],
) -> (
    ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedSemanticProjectionRootV1>,
) {
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        noop_semantic_owner(exports),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = ssa
        .source_semantic()
        .roots()
        .iter()
        .zip(exports)
        .map(|(root, export)| {
            let entry = ssa.source_semantic().functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            let workgroup = entry
                .source_contract()
                .launch()
                .unwrap()
                .required()
                .unwrap()
                .as_array();
            crate::ProductionSourceLaunchRootInputV1::new(
                export,
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some(workgroup), [1, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &inputs).unwrap();
    let roots = launch
        .roots()
        .iter()
        .zip(exports)
        .map(|(source, export)| {
            let layout = source.layout();
            let kernel = ProductionRankedKernelV1::new(
                export,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel(&format!("{export}_borrowed"), kernel)
                    .unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            ProductionRankedSemanticProjectionRootV1::new(
                source.selected_root(),
                source.source_rank(),
                lowering,
                format!("func @{export} {{\n}}\n"),
                vec![],
                vec![],
            )
        })
        .collect();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000_000);
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
    // The public materializer explicitly transfers these two receipts. Tests
    // reserve both in their caller phase; this is not whole-compiler metering.
    (owner, roots)
}

fn borrowed_n_report_fields_v1(
    report: &ProductionMirPlironTranslationValidationV1,
) -> ([u8; 32], usize, usize, usize, usize, usize) {
    (
        *report.semantic_sha256(),
        report.memory_effects(),
        report.synchronization_effects(),
        report.tensor_operations(),
        report.value_expressions(),
        report.conservative_ranked_effects,
    )
}

fn borrowed_n_retained_v1(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    owner.executable_storage().retained_storage() + owner.assert_origin_storage().payload_storage()
}

#[test]
fn borrowed_n_correspondence_keeps_exact_owner_roster_and_historical_reports() {
    for exports in [&["borrowed_single"][..], &["borrowed_z", "borrowed_a"][..]] {
        let (owner, roots) = borrowed_n_ranked_fixture_v1(exports);
        let retained = borrowed_n_retained_v1(&owner);
        let bytes = owner.executable().canonical().canonical_bytes().to_vec();
        let bytes_pointer = owner.executable().canonical().canonical_bytes().as_ptr();
        let roots_pointer = roots.as_ptr();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(retained + 19).unwrap();
        let floor = budget.storage();
        let reports = owner
            .with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |checked, _| {
                assert!(std::ptr::eq(checked.materialized(), &owner));
                assert!(std::ptr::eq(checked.executable(), owner.executable()));
                assert_eq!(
                    checked.executable().canonical().canonical_bytes().as_ptr(),
                    bytes_pointer
                );
                assert_eq!(checked.executable().canonical().canonical_bytes(), bytes);
                assert_eq!(checked.roots().as_ptr(), roots_pointer);
                assert_eq!(checked.root_count(), exports.len());
                assert!(checked.report(exports.len()).is_none());
                Ok((0..exports.len())
                    .map(|ordinal| {
                        let report = checked.report(ordinal).unwrap();
                        assert!(!report.claims_indexed_address_equivalence());
                        assert!(!report.claims_complete_operational_equivalence());
                        borrowed_n_report_fields_v1(report)
                    })
                    .collect::<Vec<_>>())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
        let work_after_borrow = budget.work();
        let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(owner, roots).unwrap();
        let attached =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
        assert_eq!(
            attached
                .pre_ranked_executable()
                .unwrap()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            bytes_pointer
        );
        assert_eq!(
            attached
                .pre_ranked_executable()
                .unwrap()
                .canonical()
                .canonical_bytes(),
            bytes
        );
        assert_eq!(
            attached
                .generic_checks
                .iter()
                .map(|check| borrowed_n_report_fields_v1(&check.translation_validation))
                .collect::<Vec<_>>(),
            reports
        );
        assert_eq!(
            budget.work(),
            work_after_borrow,
            "historical attach adds no canonical charges"
        );
        assert_eq!(budget.storage(), floor);
        drop(attached);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn borrowed_n_correspondence_rejects_roster_faults_before_new_wrapper_charges() {
    for fault in 0..6 {
        let (owner, mut roots) = borrowed_n_ranked_fixture_v1(&["roster_z", "roster_a"]);
        match fault {
            0 => roots.clear(),
            1 => {
                roots.pop();
            }
            2 => roots[1].selected_root = roots[0].selected_root,
            3 => roots.swap(0, 1),
            4 => roots[0].launch_rank = 2,
            _ => {
                let mut blocks = roots[0].lowering.kernel().blocks().to_vec();
                let ProductionRankedOperationV1::ExecutionLayout { grid_identity, .. } =
                    &mut blocks[0].operations_mut()[0]
                else {
                    panic!("exact layout first")
                };
                *grid_identity ^= 1;
                let kernel =
                    ProductionRankedKernelV1::new(roots[0].function_name(), 0, blocks).unwrap();
                roots[0].lowering = compile_ranked_kernel_for_lowering_v1(
                    ProductionConstructionV1::ranked_kernel("wrong_source_layout", kernel).unwrap(),
                    ProductionSessionLimitsV1::default(),
                )
                .unwrap();
            }
        }
        let retained = borrowed_n_retained_v1(&owner);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7);
        let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 19);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(retained + 19).unwrap();
        let floor = budget.storage();
        let mut called = false;
        let error = owner
            .with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |_, _| {
                called = true;
                Ok(())
            })
            .err()
            .unwrap();
        assert!(!called);
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported { .. }
        ));
        assert_eq!(budget.work(), 7);
        assert_eq!(budget.storage(), floor);
        let historical = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(owner, roots).err().unwrap();
        assert_eq!(error.to_string(), historical.to_string());
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn borrowed_n_correspondence_translation_failure_matches_attach_after_partial_reports() {
    let (mut owner, roots) = borrowed_n_ranked_fixture_v1(&["first_n", "second_n"]);
    // Private adversarial mutation after genuine materialization: source and N
    // bytes stay identical, but the second source/N function row is absent.
    owner.correspondence.lowered_functions = owner
        .correspondence
        .lowered_functions
        .iter()
        .filter(|row| row.correspondence_owner().index() != 1)
        .cloned()
        .collect();
    validate_source_ranked_roster_v1(&owner.semantic_ssa, &owner.source_launch, &roots).unwrap();
    let retained = borrowed_n_retained_v1(&owner);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 1_000_000);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(retained + 19).unwrap();
    let floor = budget.storage();
    let error = owner
        .with_borrowed_ranked_correspondence_v1::<()>(&roots, &mut budget, |_, _| {
            panic!("incomplete translation roster cannot enter callback")
        })
        .err()
        .unwrap();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::KernelShape
        )
    ));
    assert_eq!(budget.work(), 13); // prior7 + header2 + reserve1 + first2 + second1.
    assert_eq!(budget.storage(), floor);
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            owner, roots,
        )
        .unwrap();
    let historical = ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt)
        .err()
        .unwrap();
    assert_eq!(error.to_string(), historical.to_string());
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn borrowed_n_correspondence_callback_error_unwind_and_reentry_keep_history_and_floor() {
    let (owner, roots) = borrowed_n_ranked_fixture_v1(&["callback_n"]);
    let retained = borrowed_n_retained_v1(&owner);
    let storage_limit = retained + 1_000_000;
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(retained + 19).unwrap();
    let floor = budget.storage();
    assert!(budget.reserve_storage(storage_limit).is_err());
    let failed = budget.failed_storage();
    let sentinel = "borrowed N callback sentinel";
    let error = owner
        .with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |_, budget| {
            budget
                .reserve_storage(23)
                .map_err(SemanticKirAssertOriginErrorV1::from)?;
            Err::<(), _>(unsupported(0, None, None, sentinel))
        })
        .err()
        .unwrap();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. } if detail == sentinel)
    );
    assert_eq!(budget.storage(), floor);
    let after_error = budget.work();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), _> =
            owner.with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |_, budget| {
                budget
                    .reserve_storage(29)
                    .map_err(SemanticKirAssertOriginErrorV1::from)?;
                std::panic::panic_any(0x7135_u32)
            });
    }))
    .unwrap_err();
    assert_eq!(*panic.downcast::<u32>().unwrap(), 0x7135);
    assert!(budget.work() > after_error);
    assert_eq!(budget.storage(), floor);
    owner
        .with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |checked, _| {
            assert_eq!(checked.root_count(), 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.failed_storage(), failed);
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn borrowed_n_correspondence_wrapper_work_exact_under_and_shared_second_root_prefix() {
    for (exports, query_work) in [(&["one_work"][..], 5), (&["two_z", "two_a"][..], 7)] {
        for under in [false, true] {
            let (owner, roots) = borrowed_n_ranked_fixture_v1(exports);
            let retained = borrowed_n_retained_v1(&owner);
            let limit = 7 + 2 * query_work - usize::from(under);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 1_000_000);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(retained + 19).unwrap();
            let floor = budget.storage();
            owner
                .with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |_, _| Ok(()))
                .unwrap();
            assert_eq!(budget.work(), 7 + query_work);
            let mut called = false;
            let result =
                owner.with_borrowed_ranked_correspondence_v1(&roots, &mut budget, |_, _| {
                    called = true;
                    Ok(())
                });
            assert_eq!(called, !under);
            assert_eq!(result.is_ok(), !under);
            if under {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::AssertOrigin(
                        SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Work(_))
                    ))
                ));
            }
            assert_eq!(budget.work(), limit);
            assert_eq!(budget.storage(), floor);
            drop(owner);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 19);
            assert_eq!(work.failed_work().is_some(), under);
        }
    }
}

#[test]
fn borrowed_n_correspondence_floor_violation_precedes_callback_error_and_panic() {
    for panics in [false, true] {
        let (owner, roots) = borrowed_n_ranked_fixture_v1(&["hostile_floor_n"]);
        let retained = borrowed_n_retained_v1(&owner);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(retained + 19).unwrap();
        let floor = budget.storage();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.with_borrowed_ranked_correspondence_v1::<()>(&roots, &mut budget, |_, budget| {
                // Deliberately violate caller-owned reservation custody. The
                // scope cannot restore bytes released below its entry floor.
                let released = budget.storage() - floor + 1;
                budget
                    .release_storage(released)
                    .map_err(SemanticKirAssertOriginErrorV1::from)?;
                if panics {
                    std::panic::panic_any(0x7136_u32);
                }
                Err(unsupported(
                    0,
                    None,
                    None,
                    "must not mask accounting failure",
                ))
            })
        }));
        let result = outcome.expect("invalid cleanup takes precedence over the callback panic");
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::AssertOrigin(
                SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Accounting)
            ))
        ));
        assert_eq!(budget.storage(), floor - 1);
        assert_eq!(budget.work(), 12);
        // Repair only the deliberately removed byte for fixture teardown.
        budget.reserve_storage(1).unwrap();
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn borrowed_n_correspondence_report_storage_prefixes_drop_before_floor_restore() {
    let header = std::mem::size_of::<ProductionBorrowedRankedCorrespondenceV1<'_>>()
        + std::mem::size_of::<Vec<ProductionMirPlironTranslationValidationV1>>();
    let minimum_payload = 4 * std::mem::size_of::<ProductionMirPlironTranslationValidationV1>();
    for available in [header - 1, header + minimum_payload - 1] {
        let (owner, roots) = borrowed_n_ranked_fixture_v1(&["storage_n"]);
        let retained = borrowed_n_retained_v1(&owner);
        let floor = retained + 19;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, floor + available);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        let result =
            owner.with_borrowed_ranked_correspondence_v1::<()>(&roots, &mut budget, |_, _| {
                panic!("underpaid report/header must not enter callback")
            });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::AssertOrigin(
                SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Storage(_))
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.failed_storage().is_some());
        assert_eq!(budget.work(), if available < header { 9 } else { 10 });
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}
