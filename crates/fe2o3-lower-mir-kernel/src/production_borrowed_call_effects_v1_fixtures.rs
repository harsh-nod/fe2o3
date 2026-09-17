#[derive(Clone, Copy)]
enum BorrowedCase {
    Distinct,
    Aliased,
    WrongExtent,
    ChangedIndex,
    SameValueIndex,
    CopiedIndex,
    PlainLoad,
    VolatileLoad,
    AtomicLoad,
    Nested,
    ReboundDescriptor,
}

fn helper_read_site(case: BorrowedCase) -> ProductionSliceAccessSiteV1 {
    ProductionSliceAccessSiteV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(if matches!(case, BorrowedCase::Nested) {
            2
        } else {
            1
        }),
        SemanticBlockIdV1::from_index(1),
        Some(u32::from(matches!(
            case,
            BorrowedCase::ChangedIndex | BorrowedCase::SameValueIndex | BorrowedCase::CopiedIndex
        ))),
        0,
        SemanticBlockIdV1::from_index(0),
    )
}

fn borrowed_source(case: BorrowedCase) -> ProductionSemanticSsaOwnerV1 {
    let call = |first, second, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                vec![value(first, SLICE_REF), value(second, SLICE_REF)],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let (original, _) = fixture_with_blocks_symbol_and_slices(
        Fixture::ElidedBounds,
        false,
        |_, _| vec![block(31, vec![], SemanticTerminatorKindV1::Return)],
        |_| "borrowed_slice_root".into(),
        &[U32, U64],
        2,
    );
    let semantic = original.source_semantic();
    let root = &semantic.functions()[0];
    let read = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(
                    if matches!(
                        case,
                        BorrowedCase::SameValueIndex | BorrowedCase::CopiedIndex
                    ) {
                        7
                    } else {
                        4
                    },
                )),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        vec![
            block(
                31,
                if matches!(case, BorrowedCase::ReboundDescriptor) {
                    vec![assignment(
                        1,
                        SLICE_REF,
                        SemanticRvalueKindV1::Use(value(2, SLICE_REF)),
                    )]
                } else {
                    vec![]
                },
                call(1, 2, 1),
            ),
            block(
                32,
                vec![],
                call(
                    if matches!(case, BorrowedCase::Aliased) {
                        1
                    } else {
                        2
                    },
                    1,
                    2,
                ),
            ),
            block(33, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut reads = Vec::new();
    if matches!(case, BorrowedCase::ChangedIndex) {
        reads.push(assignment(
            4,
            U64,
            SemanticRvalueKindV1::Use(constant(U64, 1, 8)),
        ));
    }
    if matches!(
        case,
        BorrowedCase::SameValueIndex | BorrowedCase::CopiedIndex
    ) {
        reads.push(assignment(
            7,
            U64,
            SemanticRvalueKindV1::Use(if matches!(case, BorrowedCase::CopiedIndex) {
                value(4, U64)
            } else {
                constant(U64, 0, 8)
            }),
        ));
    }
    reads.push(assignment(
        6,
        U32,
        if matches!(
            case,
            BorrowedCase::PlainLoad | BorrowedCase::VolatileLoad | BorrowedCase::AtomicLoad
        ) {
            SemanticRvalueKindV1::Load(fe2o3_mir_model::semantic_mir_v1::SemanticMemoryLoadV1::new(
                read,
                if matches!(case, BorrowedCase::VolatileLoad) {
                    SemanticVolatilityV1::Volatile
                } else {
                    SemanticVolatilityV1::NonVolatile
                },
                matches!(case, BorrowedCase::AtomicLoad).then_some(
                    fe2o3_mir_model::semantic_mir_v1::SemanticAtomicAccessV1::new(
                        SemanticAtomicOrderingV1::Relaxed,
                        SemanticAtomicScopeV1::Device,
                    ),
                ),
            ))
        } else {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read))
        },
    ));
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([220; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([220; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([220; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([220; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([220; 32]),
        root.source(),
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([220; 32]),
            root.abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            2,
            root.abi().arguments().to_vec(),
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow; 2])
        .unwrap(),
        root.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                221,
                vec![
                    assignment(
                        3,
                        U64,
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: value(
                                if matches!(case, BorrowedCase::WrongExtent) {
                                    2
                                } else {
                                    1
                                },
                                SLICE_REF,
                            ),
                        },
                    ),
                    assignment(4, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                    assignment(
                        5,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: value(4, U64),
                            right: value(3, U64),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: value(5, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: value(3, U64),
                        index: value(4, U64),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(222, reads, SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let functions = if matches!(case, BorrowedCase::Nested) {
        // Source identities must follow root/wrapper/helper order without moving call targets.
        let wrapper = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([210; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([210; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([210; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([210; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([210; 32]),
            helper.source(),
            helper.abi().clone(),
            helper.locals().to_vec(),
            helper.entry(),
            vec![
                block(
                    231,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(2),
                            vec![value(1, SLICE_REF), value(2, SLICE_REF)],
                            Some(SemanticCallDestinationV1::new(
                                place(0, UNIT),
                                edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(232, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        vec![root, wrapper, helper]
    } else {
        vec![root, helper]
    };
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

// Exercise the real per-function emitter and query before helper admission.
// This fixture never constructs a production owner or changes its effect gate.
fn with_borrowed_subject(
    case: BorrowedCase,
    mutate: impl FnOnce(&mut Module, &mut SemanticKirCorrespondenceV1),
    inspect: impl FnOnce(
        CanonicalCallSubjectV1<'_>,
        SemanticKirAssertOriginsV1<'_>,
        &Inventory<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    with_borrowed_source_and_emission(case, case, mutate, inspect);
}

// Independently emit the native counterpart so lost source qualifiers remain testable.
fn with_borrowed_source_and_emission(
    case: BorrowedCase,
    emitted_case: BorrowedCase,
    mutate: impl FnOnce(&mut Module, &mut SemanticKirCorrespondenceV1),
    inspect: impl FnOnce(
        CanonicalCallSubjectV1<'_>,
        SemanticKirAssertOriginsV1<'_>,
        &Inventory<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let mut ssa = borrowed_source(case);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let semantic = ssa.source_semantic();
    let emitted_ssa = borrowed_source(emitted_case);
    let plans = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(index, source)| {
            let function = SemanticFunctionIdV1::from_index(index as u32);
            let parameters = semantic_function_parameters_v1(function, source).unwrap();
            let parameter_types = parameters
                .iter()
                .map(|(_, _, ty)| lower_parameter_type(semantic.types(), &[], *ty).unwrap())
                .collect::<Vec<_>>();
            LoweredFunctionPlanV1 {
                correspondence_owner: SemanticFunctionIdV1::from_index(0),
                semantic_function: function,
                kernel_ir_function: if index == 0 {
                    FunctionId::new("borrowed_slice_root")
                } else {
                    helper_function_id_v1(function, source)
                },
                role: if index == 0 {
                    SemanticKirFunctionRoleV1::KernelEntry
                } else {
                    SemanticKirFunctionRoleV1::InternalHelper
                },
                parameter_values: parameters
                    .iter()
                    .map(|(_, local, _)| ValueId(*local as u32))
                    .collect(),
                call_arguments: parameters
                    .iter()
                    .map(|(argument, _, _)| HelperCallArgumentV1 {
                        source_argument: *argument,
                        tuple_field: None,
                        component: None,
                        borrowed: None,
                    })
                    .collect(),
                parameter_local_bindings: parameters
                    .iter()
                    .zip(&parameter_types)
                    .map(
                        |((_, local, _), ty)| PlannedParameterLocalBindingV1::Direct {
                            local: *local,
                            value: ValueId(*local as u32),
                            ty: ty.clone(),
                        },
                    )
                    .collect(),
                parameter_declarations: parameters,
                parameter_types,
                parameter_component_bindings: vec![],
                borrowed_parameter_bindings: vec![],
                ignored_parameter_bindings: vec![],
                result_types: vec![],
            }
        })
        .collect::<Vec<_>>();
    let ids = plans
        .iter()
        .map(|plan| (plan.semantic_function, plan.kernel_ir_function.clone()))
        .collect();
    let signatures = plans
        .iter()
        .map(|plan| {
            (
                plan.semantic_function,
                LoweredFunctionSignatureV1 {
                    parameter_semantic_types: plan
                        .parameter_declarations
                        .iter()
                        .map(|(_, _, ty)| *ty)
                        .collect(),
                    call_arguments: plan.call_arguments.clone(),
                    parameter_types: plan.parameter_types.clone(),
                    result_types: vec![],
                    result_semantic_type: UNIT,
                },
            )
        })
        .collect();
    let mut emission = AssertOriginEmissionV1::new(&mut budget);
    let mut call_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut call_budget = AssertOriginBudgetV1::new(&mut call_work, STORAGE);
    let mut private = PrivateArrayLazyBudgetV1::new(1, 10_000);
    let mut calls = CallReturnBufferV1::empty();
    let mut module = Module::new("borrowed_slice_query");
    let mut correspondence = SemanticKirCorrespondenceV1 {
        borrowed_parameter_bindings: Box::default(),
        borrowed_aggregate_fields: Box::default(),
        borrowed_aggregate_calls: Box::default(),
        private_arrays: PrivateArrayCorrespondenceV1::default(),
        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
        function_count: plans.len(),
        lowered_functions: plans
            .iter()
            .map(|plan| SemanticKirFunctionCorrespondenceV1 {
                correspondence_owner: plan.correspondence_owner,
                semantic_function: plan.semantic_function,
                kernel_ir_function: plan.kernel_ir_function.clone(),
                role: plan.role,
            })
            .collect(),
        blocks: Box::default(),
        statement_operation_spans: Box::default(),
        terminator_operation_spans: Box::default(),
        generated_terminator_values: Box::default(),
        call_returns: Box::default(),
        call_result_components: Box::default(),
        synthetic_operation_spans: Box::default(),
        parameter_bindings: Box::default(),
        parameter_component_bindings: Box::default(),
        ignored_parameter_bindings: Box::default(),
    };
    let mut declarations = BTreeMap::new();
    for plan in &plans {
        let lowered = lower_one_semantic_function_v1(
            emitted_ssa.source_semantic(),
            plan,
            emitted_ssa
                .plan_for_function(plan.semantic_function)
                .unwrap(),
            &ids,
            &signatures,
            Some([64, 1, 1]),
            BTreeSet::new(),
            1,
            true,
            10_000,
            Some(&mut emission),
            &mut private,
            None,
            &mut call_budget,
        )
        .unwrap();
        macro_rules! append {
            ($field:ident) => {{
                let mut rows = std::mem::take(&mut correspondence.$field).into_vec();
                rows.extend(lowered.$field);
                correspondence.$field = rows.into_boxed_slice();
            }};
        }
        append!(blocks);
        append!(statement_operation_spans);
        append!(terminator_operation_spans);
        append!(generated_terminator_values);
        append!(synthetic_operation_spans);
        append!(parameter_bindings);
        append!(parameter_component_bindings);
        append!(ignored_parameter_bindings);
        calls
            .append(lowered.call_returns, 10_000, &mut call_budget)
            .unwrap();
        declarations.extend(lowered.diagnostic_declarations);
        declarations.extend(lowered.float_declarations);
        module.functions.push(lowered.function);
    }
    (
        correspondence.call_returns,
        correspondence.call_result_components,
    ) = calls.into_box(&mut call_budget).unwrap();
    module.functions.extend(declarations.into_values());
    module.required_capabilities = module.derived_capabilities();
    let mut kernel = Kernel::new(
        "borrowed_slice_root",
        FunctionId::new("borrowed_slice_root"),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities.extend(
        module
            .functions
            .iter()
            .flat_map(|function| function.required_capabilities.iter().cloned()),
    );
    module.kernels.push(kernel);
    mutate(&mut module, &mut correspondence);
    let (executable, graph_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            emission.budget,
        )
        .unwrap();
    emission
        .budget
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    let origins = emission.seal(&ssa, &correspondence, &executable).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&executable, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    inspect(
        CanonicalCallSubjectV1 {
            semantic_ssa: &ssa,
            executable: &executable,
            correspondence: &correspondence,
        },
        SemanticKirAssertOriginsV1 {
            executable: &executable,
            semantic_ssa: &ssa,
            origins: &origins,
        },
        &inventory,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
}
