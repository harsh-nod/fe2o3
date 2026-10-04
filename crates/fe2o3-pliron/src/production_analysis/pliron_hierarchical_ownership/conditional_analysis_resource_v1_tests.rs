#[test]
fn caller_manager_budget_is_inherited_and_full_entry_has_exact_boundaries() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut generous = manager(context, &fixture.function, unlimited()).unwrap();
    let initial = generous.resource_upper_bound();
    assert!(initial.work_upper_bound() >= 17);
    assert!(initial.retained_storage_upper_bound() >= 11);
    let report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut generous,
        &[selection],
    )
    .unwrap();
    let exact = generous.resource_upper_bound();
    assert!(exact.work_upper_bound() > initial.work_upper_bound());
    assert!(exact.retained_storage_upper_bound() > initial.retained_storage_upper_bound());
    let mut bounded = manager(
        context,
        &fixture.function,
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound(),
            exact.peak_storage_upper_bound(),
        ),
    )
    .unwrap();
    let rerun = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut bounded,
        &[selection],
    )
    .unwrap();
    assert_eq!(format!("{rerun:?}"), format!("{report:?}"));
    assert_eq!(bounded.resource_upper_bound(), exact);
    for limits in [
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound() - 1,
            exact.peak_storage_upper_bound(),
        ),
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound(),
            exact.peak_storage_upper_bound() - 1,
        ),
    ] {
        let mut bounded = match manager(context, &fixture.function, limits) {
            Ok(manager) => manager,
            Err(error) => {
                // The inherited identity capture may already set the peak.
                assert_eq!(
                    error,
                    ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::StructuralIdentity,
                        resource: "peak storage upper bound",
                    }
                );
                continue;
            }
        };
        let error = run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut bounded,
            &[selection],
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ConditionalOwnershipAnalysisErrorV1::Resource(_)
        ));
        assert!(bounded.resource_upper_bound().work_upper_bound() >= initial.work_upper_bound());
        assert!(
            bounded
                .resource_upper_bound()
                .retained_storage_upper_bound()
                >= initial.retained_storage_upper_bound()
        );
    }
    let before_repeat = generous.resource_upper_bound();
    run_conditional_ownership_analysis_v1(context, &fixture.function, &mut generous, &[selection])
        .unwrap();
    assert!(generous.resource_upper_bound().work_upper_bound() > before_repeat.work_upper_bound());
    assert!(
        generous
            .resource_upper_bound()
            .retained_storage_upper_bound()
            > before_repeat.retained_storage_upper_bound()
    );
}

#[test]
fn additional_inherited_work_and_retention_survive_as_exact_deltas() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut baseline = manager(context, &fixture.function, unlimited()).unwrap();
    let mut inherited = manager(context, &fixture.function, unlimited()).unwrap();
    let extra = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        23,
        29,
        0,
    )
    .unwrap();
    inherited
        .admit_retained_resource_upper_bound(
            ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            extra,
        )
        .unwrap();
    let baseline_report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut baseline,
        &[selection],
    )
    .unwrap();
    let inherited_report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut inherited,
        &[selection],
    )
    .unwrap();
    assert_eq!(
        format!("{baseline_report:?}"),
        format!("{inherited_report:?}")
    );
    let baseline = baseline.resource_upper_bound();
    let inherited = inherited.resource_upper_bound();
    assert_eq!(
        inherited.work_upper_bound(),
        baseline.work_upper_bound() + 23
    );
    assert_eq!(
        inherited.retained_storage_upper_bound(),
        baseline.retained_storage_upper_bound() + 29
    );
    // The extra retention starts after initial identity capture, whose earlier
    // peak need not rise. Subsequent phases must keep the extra floor live.
    assert!(inherited.peak_storage_upper_bound() >= baseline.peak_storage_upper_bound());
    assert!(inherited.peak_storage_upper_bound() <= baseline.peak_storage_upper_bound() + 29);
}

#[test]
fn unmetered_manager_and_census_substitution_refuse() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let selection = fixture.selection(context);
    let mut unmetered = PlironAnalysisManagerV1::new(&fixture.function);
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut unmetered,
            &[selection]
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "conditional ownership requires metered census",
                ..
            }
        ))
    ));
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &fixture.function);
    let capture = provider
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap();
    let mut census = capture.input_census;
    census.ownership_contracts = 0;
    let mut manager = PlironAnalysisManagerV1::new_with_resource_contract(
        &fixture.function,
        census,
        capture.resource_upper_bound,
        0,
        unlimited(),
    )
    .unwrap();
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut manager,
            &[selection]
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "conditional ownership contract census mismatch",
                ..
            }
        ))
    ));
}

#[test]
fn display_name_storage_is_prepaid_before_contract_collection() {
    let context = &mut setup();
    let fixture = fixture(context, FixtureOptions::default());
    let short_census = manager(context, &fixture.function, unlimited())
        .unwrap()
        .input_census()
        .unwrap();
    let label = "output".repeat(8_192);
    pliron::debug_info::set_operation_result_name(
        context,
        fixture.output.get_operation(),
        0,
        Some(label.as_str().try_into().unwrap()),
    );
    let mut generous = manager(context, &fixture.function, unlimited()).unwrap();
    let census = generous.input_census().unwrap();
    assert_eq!(census.identifier_bytes, short_census.identifier_bytes);
    let names = fixture
        .output
        .result(context)
        .unique_name_byte_len(context)
        .unwrap()
        * 8; // One contract and one observable write, with four copies reserved.
    let phase = ProductionAnalysisResourcePhaseV1::HierarchicalOwnership;
    let collection_admission = generous
        .resource_upper_bound()
        .checked_then_retain(pending_upper_bound(census, 1).unwrap(), phase)
        .unwrap()
        .checked_then_retain(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, names, names, 0).unwrap(),
            phase,
        )
        .unwrap();
    let mut bounded = manager(
        context,
        &fixture.function,
        ProductionAnalysisResourceLimitsV1::new(
            collection_admission.work_upper_bound() - 1,
            collection_admission.peak_storage_upper_bound(),
        ),
    )
    .unwrap();
    assert!(matches!(
        run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut bounded,
            &[fixture.selection(context)],
        ),
        Err(ConditionalOwnershipAnalysisErrorV1::Resource(
            ProductionAnalysisResourceLimitV1 {
                resource: "work upper bound",
                ..
            }
        ))
    ));
    assert_eq!(bounded.cached_entries(), 1);
    let report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut generous,
        &[fixture.selection(context)],
    )
    .unwrap();
    assert!(
        generous
            .resource_upper_bound()
            .retained_storage_upper_bound()
            >= names
    );
    assert_eq!(
        report.rows()[0].coverage(),
        Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
    );
}

#[test]
fn uncontracted_write_name_is_retained_and_prepaid_before_diagnostic_allocation() {
    let context = &mut setup();
    let mut fixture = fixture(
        context,
        FixtureOptions {
            other: Some((OwnershipCoverageAttr::ExactView, 1, 23)),
            ..FixtureOptions::default()
        },
    );
    let (contract, side) = fixture.other.take().unwrap();
    Operation::erase(contract.get_operation(), context);
    verify_operation(fixture.function.get_operation(), context).unwrap();
    pliron::debug_info::set_operation_result_name(
        context,
        side.get_operation(),
        0,
        Some("side".try_into().unwrap()),
    );
    let (short_census, short_bound) = {
        let mut manager = manager(context, &fixture.function, unlimited()).unwrap();
        let report = run_conditional_ownership_analysis_v1(
            context,
            &fixture.function,
            &mut manager,
            &[fixture.selection(context)],
        )
        .unwrap();
        assert_eq!(report.legacy_report(), &legacy(context, &fixture));
        assert!(matches!(
            report.legacy_report().findings(),
            [HierarchicalOwnershipFindingV1::UnmodeledObservableWrite { .. }]
        ));
        (
            manager.input_census().unwrap(),
            manager.resource_upper_bound(),
        )
    };

    let label = "side_write".repeat(32_768);
    pliron::debug_info::set_operation_result_name(
        context,
        side.get_operation(),
        0,
        Some(label.as_str().try_into().unwrap()),
    );
    let mut generous = manager(context, &fixture.function, unlimited()).unwrap();
    let census = generous.input_census().unwrap();
    assert_eq!(census, short_census);
    let phase = ProductionAnalysisResourcePhaseV1::HierarchicalOwnership;
    let after_scan = generous
        .resource_upper_bound()
        .checked_then_retain(pending_upper_bound(census, 1).unwrap(), phase)
        .unwrap();
    // The output appears at its contract and write, the side only at its write.
    let name_bytes = 2 * fixture
        .output
        .result(context)
        .unique_name_byte_len(context)
        .unwrap()
        + side.result(context).unique_name_byte_len(context).unwrap();
    let names = name_bytes * 4;
    let before_collection = after_scan
        .checked_then_retain(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, names, names, 0).unwrap(),
            phase,
        )
        .unwrap();
    // This long label makes the retained floor dominate the earlier fixed
    // identity workspace, so either exact one-short bound denies name credit.
    assert_eq!(
        before_collection.peak_storage_upper_bound(),
        before_collection.retained_storage_upper_bound()
    );
    for (limits, resource) in [
        (
            ProductionAnalysisResourceLimitsV1::new(
                before_collection.work_upper_bound() - 1,
                usize::MAX,
            ),
            "work upper bound",
        ),
        (
            ProductionAnalysisResourceLimitsV1::new(
                usize::MAX,
                before_collection.peak_storage_upper_bound() - 1,
            ),
            "peak storage upper bound",
        ),
    ] {
        let mut bounded = manager(context, &fixture.function, limits).unwrap();
        assert!(matches!(
            run_conditional_ownership_analysis_v1(
                context,
                &fixture.function,
                &mut bounded,
                &[fixture.selection(context)],
            ),
            Err(ConditionalOwnershipAnalysisErrorV1::Resource(error))
                if error.phase == phase && error.resource == resource
        ));
        assert_eq!(bounded.resource_upper_bound(), after_scan);
        // Only the allocation-free name scan's inventory has been prepared:
        // no contract names, prerequisite reports or retained rows were built.
        assert_eq!(bounded.cached_entries(), 1);
    }
    let before = legacy(context, &fixture);
    let report = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut generous,
        &[fixture.selection(context)],
    )
    .unwrap();
    assert_eq!(report.legacy_report(), &before);
    assert!(matches!(
        before.findings(),
        [HierarchicalOwnershipFindingV1::UnmodeledObservableWrite { view, .. }]
            if view == &side.result(context).unique_name(context).to_string()
    ));
    assert_eq!(report.prerequisites(), Check::Rejected);
    assert_eq!(report.rows().len(), 1);
    assert!(report.rows()[0].selected());
    assert_eq!(
        report.rows()[0].operation(),
        fixture.contract.get_operation()
    );
    assert_eq!(
        report.rows()[0].coverage(),
        Check::Blocked(Blocker::Prerequisite)
    );
    let exact = generous.resource_upper_bound();
    assert!(exact.work_upper_bound() > short_bound.work_upper_bound());
    assert!(
        exact.retained_storage_upper_bound()
            > short_bound.retained_storage_upper_bound() + label.len()
    );
    let mut bounded = manager(
        context,
        &fixture.function,
        ProductionAnalysisResourceLimitsV1::new(
            exact.work_upper_bound(),
            exact.peak_storage_upper_bound(),
        ),
    )
    .unwrap();
    let repeated = run_conditional_ownership_analysis_v1(
        context,
        &fixture.function,
        &mut bounded,
        &[fixture.selection(context)],
    )
    .unwrap();
    assert_eq!(format!("{repeated:?}"), format!("{report:?}"));
    assert_eq!(bounded.resource_upper_bound(), exact);
    assert_eq!(legacy(context, &fixture), before);
}

#[test]
fn contracted_alias_names_keep_complete_legacy_and_selected_row_parity() {
    let context = &mut setup();
    let fixture = fixture(
        context,
        FixtureOptions {
            other: Some((OwnershipCoverageAttr::ExactView, 1, 17)),
            ..FixtureOptions::default()
        },
    );
    let (_, alias) = fixture.other.unwrap();
    let label = "alias".repeat(8_192);
    pliron::debug_info::set_operation_result_name(
        context,
        alias.get_operation(),
        0,
        Some(label.as_str().try_into().unwrap()),
    );
    let before = legacy(context, &fixture);
    assert!(matches!(
        before.findings(),
        [HierarchicalOwnershipFindingV1::MayAliasObservableWrite {
            alias_view,
            contracted_noalias_class: 17,
            alias_noalias_class: 17,
            ..
        }] if alias_view == &alias.result(context).unique_name(context).to_string()
    ));
    let pending = analyze(context, &fixture);
    assert_eq!(pending.legacy_report(), &before);
    assert_eq!(pending.prerequisites(), Check::Rejected);
    assert_eq!(pending.rows().len(), 2);
    assert!(pending.rows()[0].selected());
    assert!(!pending.rows()[1].selected());
    assert!(
        pending
            .rows()
            .iter()
            .all(|row| { row.coverage() == Check::Blocked(Blocker::Prerequisite) })
    );
    assert_eq!(legacy(context, &fixture), before);
}
