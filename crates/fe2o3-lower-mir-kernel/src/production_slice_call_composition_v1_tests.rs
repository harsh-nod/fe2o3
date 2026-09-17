use super::*;

#[path = "production_borrowed_call_effects_v1_tests.rs"]
mod borrowed;

fn mixed_owner(budget: &mut AssertOriginBudgetV1<'_>) -> ProductionPreRankedKirOwnerV1 {
    let (original, _) = slice_source(false, false);
    let semantic = original.source_semantic();
    let root = &semantic.functions()[0];
    let source = root.source();
    let mut blocks = root.blocks().to_vec();
    let SemanticStatementKindV1::Assign(read) = blocks[1].statements()[0].kind() else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Use(operand) = read.value().kind() else {
        unreachable!()
    };
    // Keep the read in the terminator, so its source span also contains the Call.
    blocks[1] = block(
        32,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                vec![operand.clone()],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    blocks.push(block(33, vec![], SemanticTerminatorKindV1::Return));
    let root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        source,
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([220; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([220; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([220; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([220; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([220; 32]),
        source,
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([220; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                U32,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap(),
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([221; 32]),
                UNIT,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([222; 32]),
                U32,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(223, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

fn read_site() -> ProductionSliceAccessSiteV1 {
    let root = SemanticFunctionIdV1::from_index(0);
    ProductionSliceAccessSiteV1::new(
        root,
        root,
        SemanticBlockIdV1::from_index(1),
        None,
        0,
        SemanticBlockIdV1::from_index(0),
    )
}

fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &Inventory<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    owner.with_checked_canonical_calls_v1(inventory, budget, |calls, budget| {
        assert_eq!(calls.call_count(), 1);
        budget.charge_work(calls.call_count())?;
        let (root, index) = calls.sites().next().unwrap();
        let floor = budget.storage();
        for _ in 0..2 {
            calls.with_call(root, index, budget, |view| {
                assert!(std::ptr::eq(
                    view.operation(),
                    inventory.calls()[index].operation
                ));
                assert!(matches!(view.operation().kind, OperationKind::Call { .. }));
                assert_eq!(view.result_count(), 0);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            let work = budget.work();
            let result =
                owner.with_checked_slice_access_v1(inventory, read_site(), budget, |view| {
                    let call = inventory.calls()[index].coordinate;
                    let read = view.access().operation;
                    assert_eq!(read.block, call.block);
                    assert!(read.operation < call.operation);
                    assert_eq!(view.source().source_argument(), 0);
                    Ok(())
                });
            // Check the inner floor before the outer batch cleanup can conceal a leak.
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > work);
            result?;
        }
        Ok(())
    })
}

#[test]
fn slice_and_call_queries_select_distinct_operations_in_one_source_span() {
    with_owner(mixed_owner, |owner, inventory, budget| {
        inspect(owner, inventory, budget).unwrap();
        let reads = inventory
            .effects()
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Load { .. }))
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 1);
        let call = &inventory.calls()[0];
        let OperationKind::Call { arguments, .. } = &call.operation.kind else {
            unreachable!()
        };
        assert_eq!(arguments.as_slice(), &[reads[0].operation.results[0].id]);
        use fe2o3_kernel_analysis::{
            CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectsV1,
        };
        let (effects, storage) = CanonicalKirCallEffectsV1::derive(inventory, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            effects
                .decision(call.coordinate.block.function, budget)
                .unwrap(),
            Decision::CompleteNonempty
        );
        assert_eq!(
            effects.decision(call.target.unwrap(), budget).unwrap(),
            Decision::CompleteEmpty
        );
        drop(effects);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn slice_and_call_queries_share_cumulative_work_and_restore_nested_storage() {
    with_owner(mixed_owner, |owner, inventory, outer| {
        let floor = outer.storage();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        inspect(owner, inventory, &mut budget).unwrap();
        let required = (budget.work(), budget.peak_storage());
        for (work_limit, storage_limit, accepted) in [
            (required.0, required.1, true),
            (required.0 - 1, required.1, false),
            (required.0, required.1 - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = inspect(owner, inventory, &mut budget);
            match result {
                Ok(()) => assert!(accepted),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    Resource::Work(_),
                )) => assert_eq!(work_limit, required.0 - 1),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    Resource::Storage(_),
                )) => assert_eq!(storage_limit, required.1 - 1),
                Err(error) => panic!("unexpected composition failure: {error}"),
            }
            assert_eq!(budget.storage(), floor);
        }
    });
}
