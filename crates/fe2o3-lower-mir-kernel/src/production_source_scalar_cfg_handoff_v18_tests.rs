fn scalar_cfg_original_v18(changed: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = closed_owner_v1760(ClosedCaseV1760::Noop);
    let semantic = base.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, prior)| {
            let tag = if ordinal == 0 { 60 } else { 150 };
            let jump = |target| {
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(target),
                ))
            };
            let selector = SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(1, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(2),
                    ),
                )
                .unwrap(),
            };
            let statements = if changed {
                vec![assign(place(1, U32), SemanticRvalueKindV1::Use(literal(7)))]
            } else {
                vec![]
            };
            function(
                tag,
                prior.role(),
                prior.abi().clone(),
                prior.locals().to_vec(),
                vec![
                    block(tag + 4, vec![], selector),
                    block(tag + 5, statements, jump(3)),
                    block(tag + 6, vec![], jump(3)),
                    block(tag + 7, vec![], SemanticTerminatorKindV1::Return),
                ],
            )
            .with_kernel_entry(prior.kernel_entry().unwrap().clone())
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn scalar_cfg_prepared_v18(
    changed: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> (ProductionPreparedSourceV18, OriginalKernelAbiFixtureV18) {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        budget,
        || scalar_cfg_original_v18(changed),
        |owner, launch, input, _, budget| {
            let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
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
            (prepared, fixture)
        },
    )
}

#[test]
fn scalar_cfg_handoff_checks_changed_and_noop_actual_multiblock_outputs() {
    for changed in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = scalar_cfg_prepared_v18(changed, &mut budget);
        let roots = fixture.roots();
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let source_bytes = source.canonical(budget)?.canonical_bytes();
                assert!(
                    source
                        .canonical(budget)?
                        .module()
                        .functions
                        .iter()
                        .all(|function| function.body.as_ref().unwrap().blocks.len() == 4)
                );
                let handoff = source.checked_scalar_cfg_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert_eq!(output.input_audit_bytes(), source_bytes);
                assert_eq!(output.owner().canonical_bytes() != source_bytes, changed);
                assert!(
                    output
                        .owner()
                        .module()
                        .functions
                        .iter()
                        .all(|function| function.body.as_ref().unwrap().blocks.len() == 4)
                );
                assert_eq!(output.map().output_identity(), output.owner().identity());
                assert!(!output.grants_authority() && !output.execution().grants_authority());
                let header = size_of::<ProductionScalarCfgOutputHandoffV18<'_, '_>>()
                    - size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerContinuationV18>()
                    + std::mem::align_of::<ProductionScalarCfgOutputHandoffV18<'_, '_>>();
                assert_eq!(
                    handoff.retained_storage(budget)?,
                    output.storage().retained_storage() + header
                );
                assert_eq!(budget.storage(), floor + handoff.retained_storage(budget)?);
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                Ok::<_, ProductionScalarCfgHandoffErrorV18>(())
            })
            .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scalar_cfg_handoff_keeps_calls_and_assertions_unsupported() {
    for case in [ClosedCaseV1760::Call, ClosedCaseV1760::Assertion] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(case, true, &mut budget);
        let roots = fixture.roots();
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source
                .checked_scalar_cfg_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .map(|handoff| drop(handoff))
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn scalar_cfg_handoff_refuses_genuine_profiled_memory_without_exporting_output() {
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
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let error = source
            .checked_scalar_cfg_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .expect("real memory must not be relabelled scalar");
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scalar_cfg_handoff_equal_bytes_foreign_original_owner_is_not_authority() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let foreign = scalar_cfg_original_v18(false);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        assert_eq!(
            source
                .source_ssa(budget)?
                .source_semantic()
                .semantic_sha256(),
            foreign.source_semantic().semantic_sha256()
        );
        let handoff = source.checked_scalar_cfg_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let failure = handoff.check_original_source(&foreign, budget).unwrap_err();
        assert!(matches!(
            failure,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        let retained = handoff.retained_storage(budget).unwrap_err();
        assert!(matches!(
            retained,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        let cleanup = handoff.discard(budget).unwrap_err();
        assert!(matches!(
            cleanup,
            ProductionSourceOwnedViewErrorV18::Binding(_)
        ));
        Err::<(), _>(ProductionScalarCfgHandoffErrorV18::from(failure))
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scalar_cfg_handoff_foreign_ledger_preserves_cleanup_denial() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let handoff = source.checked_scalar_cfg_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
        other.reserve_storage(budget.storage())?;
        let failure = handoff.output(&other).err().unwrap();
        assert!(matches!(
            failure,
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
        let paid = budget.storage();
        assert!(handoff.discard(budget).is_err());
        assert_eq!(
            budget.storage(),
            paid,
            "lost custody cannot authorize a containing refund"
        );
        Err::<(), _>(ProductionScalarCfgHandoffErrorV18::from(failure))
    });
    assert!(result.is_err());
    assert!(budget.storage() > MODULE_FLOOR);
}

#[test]
fn scalar_cfg_handoff_preflight_work_failure_is_sticky_and_balanced() {
    let limit = 500_000_000;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = scalar_cfg_prepared_v18(false, &mut budget);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        budget.charge_work(limit - budget.work())?;
        let failure = source.checked_scalar_cfg_output_v18(ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).err().unwrap();
        let ProductionScalarCfgHandoffErrorV18::Check(ProductionScalarCfgCheckErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &failure else { panic!("wrong first refusal: {failure:?}") };
        assert!(matches!(resource, ArgumentResourceV1::Work(error) if error.actual() == limit + 1 && error.limit() == limit));
        let retry = source.checked_scalar_cfg_output_v18(ProductionKernelArgumentAbiInputV18 { roots: &roots }, budget).err().unwrap();
        assert!(matches!(retry, ProductionScalarCfgHandoffErrorV18::Check(ProductionScalarCfgCheckErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(again))) if again == *resource));
        assert_eq!(budget.work(), limit);
        Err::<(), _>(failure)
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn scalar_cfg_handoff_exact_and_one_short_storage_use_actual_cfg_pipeline() {
    fn run(
        limit: usize,
    ) -> (
        Result<(), ProductionScalarCfgHandoffErrorV18>,
        usize,
        Option<usize>,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = scalar_cfg_prepared_v18(true, &mut budget);
        let roots = fixture.roots();
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            source
                .checked_scalar_cfg_output_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?
                .discard(budget)?;
            Ok::<_, ProductionScalarCfgHandoffErrorV18>(())
        });
        assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
        (result, budget.peak_storage(), budget.failed_storage())
    }
    let (result, peak, failed) = run(MODULE_LIMIT);
    result.unwrap();
    assert_eq!(failed, None);
    let (result, exact_peak, failed) = run(peak);
    result.unwrap();
    assert_eq!((exact_peak, failed), (peak, None));
    let (result, refused_peak, failed) = run(peak - 1);
    let error = result.expect_err("one-short must refuse actual storage");
    assert!(
        matches!(closed_resource_cause_v1762(&error), Some(ArgumentResourceV1::Storage(error))
        if (error.actual(), error.limit()) == (peak, peak - 1))
    );
    assert_eq!(failed, Some(peak));
    assert!(refused_peak < peak);
}
