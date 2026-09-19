use super::*;
pub(super) mod transport {
    include!("production_execution_owner_transport_v29_tests.rs");
}

#[derive(Clone, Copy, Debug)]
pub(super) enum OwnerParameterCase {
    Valid,
    ByValue,
    Immutable,
    PaddedMemory,
}

pub(super) fn owner_parameter_fixture(
    case: OwnerParameterCase,
) -> (ProductionSemanticSsaOwnerV1, SemanticTypeIdV1) {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let pointer = reference(
        &mut types,
        U32,
        if matches!(case, OwnerParameterCase::Immutable) {
            SemanticMutabilityV1::Immutable
        } else {
            SemanticMutabilityV1::Mutable
        },
        true,
    );
    let backend = *types[pointer.index() as usize].layout().backend_repr();
    let properties = types[pointer.index() as usize].abi_properties();
    let padded = matches!(case, OwnerParameterCase::PaddedMemory);
    let carrier = aggregate(
        &mut types,
        vec![pointer],
        vec![0],
        if padded { 16 } else { 8 },
        8,
        if padded {
            SemanticBackendReprV1::memory(true)
        } else {
            backend
        },
        None,
    );
    if !padded {
        types[carrier.index() as usize] = types[carrier.index() as usize]
            .clone()
            .with_rustc_abi_properties(properties);
    }
    let make_abi = |tag, kernel, inputs: &[SemanticTypeIdV1], ownership| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
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
            inputs.len() as u32,
            inputs
                .iter()
                .map(|ty| SemanticAbiArgumentV1::source(value_abi(&types, *ty)))
                .collect(),
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap()
    };
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    assert_eq!(locals.len(), 3);
    locals.push(local(162, carrier, SemanticLocalRoleV1::Argument(1)));
    let mut blocks = root.blocks().to_vec();
    blocks[1] = block(
        86,
        vec![],
        call(
            1,
            vec![
                SemanticOperandV1::Move(place(2, CONTEXT)),
                SemanticOperandV1::Move(place(3, carrier)),
                SemanticOperandV1::Copy(place(1, U32)),
            ],
            2,
        ),
    );
    functions[0] = function(
        80,
        root.role(),
        make_abi(
            81,
            true,
            &[U32, carrier],
            vec![
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ],
        ),
        locals,
        blocks,
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let provider = &functions[1];
    let mut locals = provider.locals().to_vec();
    assert_eq!(locals.len(), 6);
    locals[2] = local(104, U32, SemanticLocalRoleV1::Argument(2));
    locals.push(local(163, carrier, SemanticLocalRoleV1::Argument(1)));
    functions[1] = function(
        100,
        provider.role(),
        make_abi(
            101,
            false,
            &[CONTEXT, carrier, U32],
            vec![
                SemanticSourceArgumentOwnershipV1::ByValue,
                if matches!(case, OwnerParameterCase::ByValue) {
                    SemanticSourceArgumentOwnershipV1::ByValue
                } else {
                    SemanticSourceArgumentOwnershipV1::ExclusiveOwner
                },
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        ),
        locals,
        provider.blocks().to_vec(),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    (owner, carrier)
}

fn with_owner_instances<R>(
    mut owner: ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    consumer: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> (Result<R, ProductionSemanticKirErrorV1>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(FLOOR)?;
        let captured = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|error| match error {
                fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        budget.reserve_storage(captured.retained_storage())?;
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consumer(
                instances, budget,
            ))
        })
        .map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                error.into()
            }
            _ => execution_call_error_v29(),
        })?
    })();
    let peak = budget.peak_storage();
    (result, work.work(), peak)
}

fn signature_bytes(s: &LoweredFunctionSignatureV1) -> usize {
    s.parameter_semantic_types.capacity() * std::mem::size_of::<SemanticTypeIdV1>()
        + s.call_arguments.capacity() * std::mem::size_of::<HelperCallArgumentV1>()
        + s.parameter_types.capacity() * std::mem::size_of::<Type>()
        + s.result_types.capacity() * std::mem::size_of::<Type>()
        + std::mem::size_of::<Type>()
}

fn plan_bytes(p: &LoweredFunctionPlanV1) -> usize {
    p.parameter_declarations.capacity() * std::mem::size_of::<(u32, usize, SemanticTypeIdV1)>()
        + p.parameter_types.capacity() * std::mem::size_of::<Type>()
        + p.parameter_values.capacity() * std::mem::size_of::<ValueId>()
        + p.call_arguments.capacity() * std::mem::size_of::<HelperCallArgumentV1>()
        + p.result_types.capacity() * std::mem::size_of::<Type>()
        + std::mem::size_of::<Type>()
}

fn selectors(s: &[HelperCallArgumentV1]) -> Vec<(u32, Option<u32>, Option<usize>)> {
    s.iter()
        .map(|r| (r.source_argument, r.tuple_field, r.component))
        .collect()
}

fn child(instances: &ExecutionInstancesV29<'_>) -> ProductionCallInstanceIdV1 {
    instances
        .calls(instances.root())
        .unwrap()
        .iter()
        .find_map(|call| call.child())
        .unwrap()
}

#[test]
fn owner_parameter_preserves_source_roster_and_entry_local_order() {
    let (owner, carrier) = owner_parameter_fixture(OwnerParameterCase::Valid);
    with_owner_instances(owner, 10_000_000, 10_000_000, |instances, budget| {
        let floor = budget.storage();
        let signature = execution_function_signature_v29(instances, child(instances), budget)?;
        assert_eq!(signature.parameter_semantic_types, [CONTEXT, carrier, U32]);
        assert_eq!(
            signature.parameter_types,
            [
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite
                )
            ]
        );
        assert_eq!(
            selectors(&signature.call_arguments),
            [(2, None, Some(0)), (1, None, None)]
        );
        let kept = signature_bytes(&signature);
        assert_eq!(budget.storage() - floor, kept);
        let plan = execution_instance_plan_v29(
            instances,
            child(instances),
            FunctionId::new("owner_parameter"),
            SemanticEmissionPlacementV1 {
                first_block: 17,
                first_value: 300,
            },
            budget,
        )?;
        assert_eq!(plan.parameter_types, signature.parameter_types);
        assert_eq!(plan.parameter_values, [ValueId(300), ValueId(301)]);
        assert_eq!(
            plan.parameter_declarations,
            [(0, 1, CONTEXT), (2, 2, U32), (1, 6, carrier)]
        );
        assert_eq!(
            selectors(&plan.call_arguments),
            selectors(&signature.call_arguments)
        );
        let total = kept + plan_bytes(&plan);
        assert_eq!(budget.storage() - floor, total);
        drop((signature, plan));
        budget.release_storage(total)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn owner_parameter_rejects_unowned_and_nontransparent_source_abis() {
    for case in [
        OwnerParameterCase::ByValue,
        OwnerParameterCase::Immutable,
        OwnerParameterCase::PaddedMemory,
    ] {
        let (owner, _) = owner_parameter_fixture(case);
        with_owner_instances(owner, 10_000_000, 10_000_000, |instances, budget| {
            let floor = budget.storage();
            assert!(
                matches!(
                    execution_function_signature_v29(instances, child(instances), budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "{case:?}"
            );
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn failed_owner_parameter_plan_preserves_existing_boxed_signature() {
    let (owner, _) = owner_parameter_fixture(OwnerParameterCase::Valid);
    with_owner_instances(owner, 10_000_000, 10_000_000, |instances, budget| {
        let signature = execution_function_signature_v29(instances, child(instances), budget)?;
        let floor = budget.storage();
        assert!(matches!(
            execution_instance_plan_v29(
                instances,
                child(instances),
                FunctionId::new("exhausted_owner"),
                SemanticEmissionPlacementV1 {
                    first_block: 0,
                    first_value: u32::MAX
                },
                budget
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Arithmetic
                )
            )
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            selectors(&signature.call_arguments),
            [(2, None, Some(0)), (1, None, None)]
        );
        let retained = signature_bytes(&signature);
        drop(signature);
        budget.release_storage(retained)?;
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn owner_parameter_authentication_has_exact_and_short_resource_boundaries() {
    let (owner, carrier) = owner_parameter_fixture(OwnerParameterCase::Valid);
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = execution_direct_parameter_v29(
            owner.source_semantic(),
            HELPER,
            1,
            None,
            carrier,
            &mut budget,
        );
        let peak = budget.peak_storage();
        let error = match result {
            Ok(Some(ty)) => {
                assert_eq!(budget.storage(), FLOOR + std::mem::size_of::<Type>());
                drop(ty);
                budget.release_storage(std::mem::size_of::<Type>()).unwrap();
                None
            }
            Ok(None) => panic!("owner was not authenticated"),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) => Some(error),
            Err(error) => panic!("unexpected classifier error: {error:?}"),
        };
        assert_eq!(budget.storage(), FLOOR);
        (error, work.work(), peak)
    };
    let (error, work, peak) = run(10_000_000, 10_000_000);
    assert!(error.is_none());
    assert_eq!(run(work, peak), (None, work, peak));
    assert!(matches!(
        run(work - 1, peak).0,
        Some(ArgumentResourceV1::Work(_))
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Some(ArgumentResourceV1::Storage(_))
    ));
}

#[test]
fn owner_parameter_caller_rejects_mixed_duplicate_and_wrong_types() {
    let (owner, carrier) = owner_parameter_fixture(OwnerParameterCase::Valid);
    let expected = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let binding = SemanticValueBindingV1::Value {
        id: ValueId(42),
        ty: expected.clone(),
    };
    let direct = HelperCallArgumentV1 {
        source_argument: 1,
        tuple_field: None,
        component: None,
    };
    let component = HelperCallArgumentV1 {
        component: Some(0),
        ..direct.clone()
    };
    let tuple = HelperCallArgumentV1 {
        tuple_field: Some(0),
        ..direct.clone()
    };
    let run = |projections: &[HelperCallArgumentV1], physical: &[Type]| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
        execution_call_argument_shape_v29(
            owner.source_semantic().types(),
            carrier,
            1,
            &binding,
            projections,
            physical,
            &mut budget,
        )
    };
    run(&[direct.clone()], &[expected.clone()]).unwrap();
    for projections in [
        vec![direct.clone(), direct.clone()],
        vec![direct.clone(), component],
        vec![tuple],
        vec![],
    ] {
        let physical = vec![expected.clone(); projections.len()];
        assert!(run(&projections, &physical).is_err());
    }
    for physical in [
        Type::Scalar(ScalarType::U32),
        Type::pointer(
            Type::Scalar(ScalarType::U64),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
    ] {
        assert!(run(&[direct.clone()], &[physical]).is_err());
    }
}
