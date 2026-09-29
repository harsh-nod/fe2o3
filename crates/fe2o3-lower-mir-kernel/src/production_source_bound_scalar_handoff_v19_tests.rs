fn bound_launches_v19(count: usize) -> Vec<fe2o3_kernel_ir::CanonicalFormalLaunchInputV19> {
    vec![
        fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::PhysicalEnvelope(
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [192, 1, 1]
            },
        );
        count
    ]
}

fn bound_has_obligations_v19(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if matches!(
            error.downcast_ref::<ProductionBoundScalarResidualV19>(),
            Some(ProductionBoundScalarResidualV19::Obligations)
        ) {
            return true;
        }
        current = error.source();
    }
    false
}

#[test]
fn bound_scalar_handoff_retains_changed_and_noop_cfg_under_static_padding_and_dynamic_domain() {
    for changed in [false, true] {
        for static_domain in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let (prepared, fixture) =
                scalar_cfg_prepared_geometry_v18(changed, static_domain, &mut budget);
            let roots = fixture.roots();
            let completed = std::cell::Cell::new(false);
            prepared
                .with_source_consumer_v18(&mut budget, |source, budget| {
                    let floor = budget.storage();
                    let launches = bound_launches_v19(source.root_count(budget)?);
                    let before = source.canonical(budget)?;
                    let original_bytes = before.canonical_bytes();
                    let handoff = source.checked_bound_scalar_output_v19(
                        ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        &launches,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        budget,
                    )?;
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    handoff.check_reported_owner_v19(before, budget)?;
                    let output = handoff.output(budget)?;
                    handoff.check_reported_owner_v19(output.owner(), budget)?;
                    assert_eq!(output.input_audit_bytes(), original_bytes);
                    assert_eq!(output.owner().canonical_bytes() != original_bytes, changed);
                    assert_eq!(output.map().output_identity(), output.owner().identity());
                    assert!(!output.grants_authority());
                    assert_eq!(
                        handoff.formal_context_v19(budget)?,
                        (
                            launches.as_slice(),
                            fe2o3_kernel_ir::FormalIndexWidth::Bits64
                        )
                    );
                    assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                    handoff.discard(budget)?;
                    assert_eq!(budget.storage(), floor);
                    completed.set(true);
                    Ok::<_, ProductionBoundScalarHandoffErrorV19>(())
                })
                .unwrap();
            assert!(completed.get());
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn bound_scalar_handoff_requires_complete_launch_roster_and_known_formal_coverage() {
    for missing in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = scalar_cfg_prepared_geometry_v18(false, false, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let error = prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let launches = if missing {
                    vec![]
                } else {
                    vec![
                        fe2o3_kernel_ir::CanonicalFormalLaunchInputV19::Exact(
                            fe2o3_kernel_ir::ExplicitLaunchExtent::Unknown
                        );
                        source.root_count(budget)?
                    ]
                };
                let error = source
                    .checked_bound_scalar_output_v19(
                        ProductionKernelArgumentAbiInputV18 { roots: &roots },
                        &launches,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        budget,
                    )
                    .err()
                    .expect("unresolved coverage cannot retain checked output");
                if missing {
                    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&error);
                    let mut exact = false;
                    while let Some(value) = cause {
                        exact |= matches!(
                            value.downcast_ref::<ProductionSourceOwnedViewErrorV18>(),
                            Some(ProductionSourceOwnedViewErrorV18::Binding(
                                "paired formal report complete launch roster"
                            ))
                        );
                        cause = value.source();
                    }
                    assert!(exact, "{error:?}");
                } else {
                    assert!(bound_has_obligations_v19(&error), "{error:?}");
                }
                completed.set(true);
                Err::<(), _>(error)
            })
            .unwrap_err();
        assert!(completed.get(), "{error:?}");
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn bound_scalar_handoff_rejects_actual_memory_instead_of_clearing_its_report() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        || descriptor_source_owner(DescriptorCase::READ),
        |owner, launch, input, _, budget| {
            let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = fixture.roots();
            let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap();
            drop(roots);
            (prepared, fixture)
        },
    );
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let error = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = bound_launches_v19(source.root_count(budget)?);
            let error = source
                .checked_bound_scalar_output_v19(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    &launches,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    budget,
                )
                .err()
                .expect("actual memory cannot become memory-free output");
            assert!(bound_has_obligations_v19(&error), "{error:?}");
            completed.set(true);
            Err::<(), _>(error)
        })
        .unwrap_err();
    assert!(completed.get(), "{error:?}");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn bound_scalar_handoff_equal_bytes_foreign_original_remains_refused() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let foreign = scalar_cfg_original_v18(false);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let error = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            assert_eq!(
                source
                    .source_ssa(budget)?
                    .source_semantic()
                    .semantic_sha256(),
                foreign.source_semantic().semantic_sha256()
            );
            let launches = bound_launches_v19(source.root_count(budget)?);
            let handoff = source.checked_bound_scalar_output_v19(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            let failure = handoff.check_original_source(&foreign, budget).unwrap_err();
            assert!(matches!(
                failure,
                ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
            ));
            assert!(matches!(
                handoff.output(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "foreign original SSA owner"
                ))
            ));
            assert!(matches!(
                handoff.discard(budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "foreign original SSA owner"
                ))
            ));
            completed.set(true);
            Err::<(), _>(ProductionBoundScalarHandoffErrorV19::from(failure))
        })
        .unwrap_err();
    assert!(completed.get(), "{error:?}");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn bound_scalar_handoff_rejects_equal_bytes_rebuilt_report_owners() {
    for output in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = scalar_cfg_prepared_v18(true, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let error = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = bound_launches_v19(source.root_count(budget)?);
            let handoff = source.checked_bound_scalar_output_v19(
                ProductionKernelArgumentAbiInputV18 { roots: &roots }, &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64, budget,
            )?;
            let owner = if output { handoff.output(budget)?.owner() } else { source.canonical(budget)? };
            let layouts = source.limits(budget)?.storage_layout_limits();
            let (other, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                owner.module(), layouts, budget,
            ).unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            assert_eq!(other.canonical_bytes(), owner.canonical_bytes());
            assert!(!std::ptr::eq(&other, owner));
            let error = handoff.check_reported_owner_v19(&other, budget).unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding("bound scalar formal owner differs")));
            drop(other);
            budget.release_storage(receipt.retained_storage())?;
            assert!(matches!(handoff.output(budget), Err(ProductionSourceOwnedViewErrorV18::Binding("bound scalar formal owner differs"))));
            assert!(matches!(handoff.discard(budget), Err(ProductionSourceOwnedViewErrorV18::Binding("bound scalar formal owner differs"))));
            completed.set(true);
            Err::<(), _>(ProductionBoundScalarHandoffErrorV19::from(error))
        }).unwrap_err();
        assert!(completed.get(), "{error:?}");
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn bound_scalar_handoff_foreign_ledger_does_not_refund_original_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let error = prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = bound_launches_v19(source.root_count(budget)?);
            let handoff = source.checked_bound_scalar_output_v19(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            let paid = budget.storage();
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
            let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
            other.reserve_storage(paid)?;
            let failure = handoff.formal_context_v19(&other).unwrap_err();
            assert!(matches!(
                failure,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            assert!(handoff.discard(budget).is_err());
            assert_eq!(budget.storage(), paid);
            completed.set(true);
            Err::<(), _>(ProductionBoundScalarHandoffErrorV19::from(failure))
        })
        .unwrap_err();
    assert!(completed.get(), "{error:?}");
    assert!(budget.storage() > MODULE_FLOOR);
}

#[test]
fn bound_scalar_handoff_preflight_denial_is_sticky_before_any_report() {
    let limit = 500_000_000;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let error = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let launches = bound_launches_v19(source.root_count(budget)?);
        budget.charge_work(limit - budget.work())?;
        let failure = source.checked_bound_scalar_output_v19(
            ProductionKernelArgumentAbiInputV18 { roots: &roots }, &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64, budget,
        ).err().unwrap();
        let ProductionBoundScalarHandoffErrorV19::Check(ProductionBoundScalarCheckErrorV19::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &failure else { panic!("wrong first refusal: {failure:?}") };
        assert!(matches!(resource, ArgumentResourceV1::Work(value) if value.actual() == limit + 1 && value.limit() == limit));
        let retry = source.checked_bound_scalar_output_v19(
            ProductionKernelArgumentAbiInputV18 { roots: &roots }, &launches,
            fe2o3_kernel_ir::FormalIndexWidth::Bits64, budget,
        ).err().unwrap();
        assert!(matches!(retry, ProductionBoundScalarHandoffErrorV19::Check(ProductionBoundScalarCheckErrorV19::Source(ProductionSourceOwnedViewErrorV18::Resource(again))) if again == *resource));
        completed.set(true);
        Err::<(), _>(failure)
    }).unwrap_err();
    assert!(completed.get(), "{error:?}");
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn bound_scalar_handoff_exact_and_one_short_storage_cover_real_reports_and_native_checks() {
    fn run(
        limit: usize,
    ) -> (
        Result<(), ProductionBoundScalarHandoffErrorV19>,
        usize,
        bool,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = scalar_cfg_prepared_v18(true, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = bound_launches_v19(source.root_count(budget)?);
            let handoff = source.checked_bound_scalar_output_v19(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            handoff.discard(budget)?;
            completed.set(true);
            Ok::<_, ProductionBoundScalarHandoffErrorV19>(())
        });
        assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
        (result, budget.peak_storage(), completed.get())
    }
    let (result, peak, completed) = run(MODULE_LIMIT);
    result.unwrap();
    assert!(completed && peak > MODULE_FLOOR);
    let (result, exact_peak, completed) = run(peak);
    result.unwrap();
    assert!(completed);
    assert_eq!(exact_peak, peak);
    let (result, refused_peak, completed) = run(peak - 1);
    let error = result.unwrap_err();
    assert!(!completed);
    let resource = closed_resource_cause_v1762(&error).expect("exact typed storage refusal");
    assert!(
        matches!(resource, ArgumentResourceV1::Storage(value) if value.actual() == peak && value.limit() == peak - 1),
        "{error:?}"
    );
    assert!(refused_peak < peak);
}
