use super::*;

const LIMIT: usize = 20_000_000;

fn mixed_pointer_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = nominal_owner(Input::CapturedWorkgroupReference, false);
    let source = original.source_semantic();
    let mixed = source.functions()[0].locals()[1].ty();
    let mut types = source.types().to_vec();
    let pointer = reference(&mut types, mixed, SemanticMutabilityV1::Immutable, false);
    let make_abi = |tag, kernel| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            source.target_layout_identity(),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value_abi(&types, pointer))],
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
        .unwrap()
    };
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        make_abi(203, true),
        vec![
            local(210, UNIT, SemanticLocalRoleV1::Return),
            local(211, pointer, SemanticLocalRoleV1::Argument(0)),
            local(212, U32, SemanticLocalRoleV1::Temporary),
            local(213, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                220,
                vec![],
                call(1, vec![SemanticOperandV1::Move(place(1, pointer))], 1),
            ),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(source.functions()[0].kernel_entry().unwrap().clone());
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(205, false),
        vec![
            local(230, UNIT, SemanticLocalRoleV1::Return),
            local(231, pointer, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(240, vec![], SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        source.callables().to_vec(),
        vec![ROOT],
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

fn with_original_mixed_c1(mut owner: ProductionSemanticSsaOwnerV1, check_abi: bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let mixed = owner.source_semantic().functions()[0].locals()[1].ty();
    let types = demands.types(&owner, &mut budget).unwrap();
    assert!(
        !types.contains(&mixed),
        "a transported nominal holder is not a whole backing layout"
    );
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        types,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let table = budget.storage();
    // Failure to find a whole physical layout is an explicit refusal, never a
    // complete-support marker. The original C1 and ABI paths remain separate.
    assert!(layouts.row_for(&owner, mixed, &mut budget).is_err());
    let entered =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, _, budget| {
                    plan.check_owner(instances, budget)?;
                    assert!(plan.storage_root.is_some());
                    if check_abi {
                        let child = instances.calls(instances.root()).unwrap()[0]
                            .child()
                            .unwrap();
                        let before = budget.storage();
                        let abi = execution_instance_plan_v29(
                            instances,
                            child,
                            FunctionId::new("transported_mixed"),
                            SemanticEmissionPlacementV1 {
                                first_block: 17,
                                first_value: 300,
                            },
                            budget,
                        )?;
                        assert_eq!(abi.parameter_types, [Type::Scalar(ScalarType::U32)]);
                        assert_eq!(abi.call_arguments.len(), 1);
                        assert_eq!(abi.call_arguments[0].component, Some(0));
                        assert!(abi.parameter_local_bindings.is_empty());
                        drop(abi);
                        budget.release_storage(budget.storage() - before)?;
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(true)
        })
        .unwrap();
    assert!(entered);
    assert_eq!(budget.storage(), table);
    layouts.release(&mut budget).unwrap();
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn mixed_nominal_holders_survive_real_c1_planning_and_existing_abi_transport() {
    for input in [Input::CapturedWorkgroupReference, Input::CapturedContext] {
        with_original_mixed_c1(nominal_owner(input, false), true);
    }
}

#[test]
fn mixed_pointer_pointee_absence_is_not_whole_backing_authority() {
    with_original_mixed_c1(mixed_pointer_owner(), false);
}
