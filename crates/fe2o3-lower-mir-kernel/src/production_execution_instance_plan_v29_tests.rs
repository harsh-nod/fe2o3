use super::*;

fn plan_case(
    mut owner: ProductionSemanticSsaOwnerV1,
    first_value: u32,
    root: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<LoweredFunctionPlanV1, ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
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
        let prior = budget.storage();
        let result =
            with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
                let instance = if root {
                    instances.root()
                } else {
                    instances.calls(instances.root()).unwrap()[0]
                        .child()
                        .unwrap()
                };
                let floor = budget.storage();
                let result = execution_instance_plan_v29(
                    instances,
                    instance,
                    FunctionId::new("planned_child"),
                    SemanticEmissionPlacementV1 {
                        first_block: 17,
                        first_value,
                    },
                    budget,
                );
                if let Ok(plan) = &result {
                    let retained = plan.parameter_declarations.capacity()
                        * std::mem::size_of::<(u32, usize, SemanticTypeIdV1)>()
                        + plan.parameter_types.capacity() * std::mem::size_of::<Type>()
                        + plan.parameter_values.capacity() * std::mem::size_of::<ValueId>()
                        + plan.call_arguments.capacity()
                            * std::mem::size_of::<HelperCallArgumentV1>()
                        + plan.result_types.capacity() * std::mem::size_of::<Type>();
                    assert_eq!(
                        budget.storage() - floor,
                        retained,
                        "temporary ABI/physical vectors must not remain charged"
                    );
                } else {
                    assert_eq!(
                        budget.storage(),
                        floor,
                        "failed plan must release its own allocations"
                    );
                }
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            })
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        if result.is_err() {
            assert_eq!(budget.storage(), prior);
        }
        result
    })();
    let peak = budget.peak_storage();
    let live = budget.storage();
    (result, work.work(), peak, live)
}

#[test]
fn instance_plan_preserves_packed_and_expanded_source_selectors() {
    for shape in [
        Shape::Tuple,
        Shape::Struct,
        Shape::RustCall,
        Shape::IndexTuple,
        Shape::IndexRustCall,
        Shape::PackedRustCall,
    ] {
        let plan = plan_case(owner(shape), 300, false, 10_000_000, 10_000_000)
            .0
            .unwrap();
        assert_eq!(plan.correspondence_owner, ROOT);
        assert_eq!(plan.semantic_function, HELPER);
        assert_eq!(plan.parameter_types, [shape.physical()]);
        assert_eq!(plan.parameter_values, [ValueId(300)]);
        assert_eq!(plan.result_types, [shape.physical()]);
        assert_eq!(plan.call_arguments.len(), 1);
        let selector = &plan.call_arguments[0];
        assert_eq!(selector.source_argument, u32::from(shape.rust_call()));
        assert_eq!(selector.tuple_field, shape.expanded().then_some(1));
        assert_eq!(selector.component, Some(0));
        assert_eq!(
            plan.parameter_declarations.len(),
            if shape.expanded() {
                3
            } else if shape.rust_call() {
                2
            } else {
                1
            }
        );
        assert!(plan.parameter_local_bindings.is_empty());
        assert!(plan.parameter_component_bindings.is_empty());
        assert!(plan.ignored_parameter_bindings.is_empty());
    }
}

#[test]
fn instance_plan_checks_identity_space_and_root_without_leaking_storage() {
    assert!(matches!(
        plan_case(
            owner(Shape::RustCall),
            u32::MAX,
            false,
            10_000_000,
            10_000_000
        )
        .0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Arithmetic
            )
        )
    ));
    assert!(
        plan_case(owner(Shape::RustCall), 300, true, 10_000_000, 10_000_000)
            .0
            .is_err()
    );
}

#[test]
fn instance_plan_has_exact_work_and_storage_boundaries() {
    let (result, work, storage, _) = plan_case(
        owner(Shape::PackedRustCall),
        300,
        false,
        10_000_000,
        10_000_000,
    );
    result.unwrap();
    assert!(
        plan_case(owner(Shape::PackedRustCall), 300, false, work, storage)
            .0
            .is_ok()
    );
    assert!(
        plan_case(owner(Shape::PackedRustCall), 300, false, work - 1, storage)
            .0
            .is_err()
    );
    assert!(
        plan_case(owner(Shape::PackedRustCall), 300, false, work, storage - 1)
            .0
            .is_err()
    );
}

#[derive(Clone, Copy, Debug)]
enum Input {
    MutableContext,
    Workgroup,
    SharedWorkgroup,
    CapturedContext,
    CapturedWorkgroup,
    CapturedWorkgroupReference,
    RawContext,
}

fn declaration(
    types: &mut Vec<SemanticTypeDeclV1>,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
    role: Option<SemanticExecutionRoleV29>,
) -> SemanticTypeIdV1 {
    let id = SemanticTypeIdV1::from_index(types.len() as u32);
    let tag = 130 + types.len() as u8;
    let mut declaration = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    );
    if let Some(role) = role {
        declaration = declaration.with_rust_type_kind(SemanticRustTypeKindV1::Execution(role));
    }
    types.push(declaration);
    id
}

fn aggregate(
    types: &mut Vec<SemanticTypeDeclV1>,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    size: u64,
    alignment: u64,
    backend: SemanticBackendReprV1,
    role: Option<SemanticExecutionRoleV29>,
) -> SemanticTypeIdV1 {
    let used = fields
        .iter()
        .zip(&offsets)
        .map(|(ty, offset)| offset + types[ty.index() as usize].layout().size_bytes().unwrap())
        .max()
        .unwrap_or(0);
    let padding = if used < size {
        vec![SemanticPaddingV1::new(used, size - used).unwrap()]
    } else {
        vec![]
    };
    declaration(
        types,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(size),
            alignment,
            backend,
            false,
            SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        role,
    )
}

fn reference(
    types: &mut Vec<SemanticTypeDeclV1>,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
    raw: bool,
) -> SemanticTypeIdV1 {
    let layout = types[pointee.index() as usize].layout();
    let info = SemanticAbiPointeeInfoV1::new(
        if mutability == SemanticMutabilityV1::Mutable {
            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
        } else {
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        },
        layout.size_bytes().unwrap(),
        layout.alignment_bytes(),
    )
    .unwrap();
    let id = declaration(
        types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(u128::from(!raw), u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                if raw {
                    SemanticPointerKindV1::Raw
                } else {
                    SemanticPointerKindV1::Reference
                },
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        None,
    );
    if !raw {
        types[id.index() as usize] = types[id.index() as usize]
            .clone()
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false)
                    .with_scalar_pointee_info(Some(info), None),
            );
    }
    id
}

fn value_abi(types: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let ty = &types[id.index() as usize];
    let scalar = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let mode = if ty.layout().size_bytes() == Some(0) {
        SemanticAbiPassModeV1::Ignore
    } else {
        match ty.layout().backend_repr() {
            SemanticBackendReprV1::Scalar(_) => {
                let attributes = match ty.abi_properties().first_pointee() {
                    Some(info) => {
                        let shared = matches!(
                            info.kind(),
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        );
                        SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                true,
                                shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                                true,
                                shared,
                                false,
                                true,
                            ),
                            SemanticAbiExtensionV1::None,
                            info.guaranteed_size_bytes(),
                            (info.reliable_alignment_bytes() > 1)
                                .then_some(info.reliable_alignment_bytes()),
                        )
                        .unwrap()
                    }
                    None => scalar,
                };
                SemanticAbiPassModeV1::Direct(attributes)
            }
            SemanticBackendReprV1::ScalarPair { .. } => SemanticAbiPassModeV1::Pair {
                first: scalar,
                second: scalar,
            },
            SemanticBackendReprV1::Memory { .. } => SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesNone),
                        true,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    ty.layout().size_bytes().unwrap(),
                    Some(ty.layout().alignment_bytes()),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            },
            _ => panic!("unexpected fixture carrier"),
        }
    };
    SemanticAbiValueV1::new(id, mode)
}

// Complete inert source declarations, including the real nonzero WG carrier.
fn nominal_owner(input: Input, wrong_ownership: bool) -> ProductionSemanticSsaOwnerV1 {
    let mut types = execution_owner(Flow::Linear)
        .unwrap()
        .source_semantic()
        .types()
        .to_vec();
    let workgroup = if matches!(
        input,
        Input::Workgroup
            | Input::SharedWorkgroup
            | Input::CapturedWorkgroup
            | Input::CapturedWorkgroupReference
    ) {
        let scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let index = declaration(
            &mut types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
            None,
        );
        let marker = SemanticTypeIdV1::from_index(2);
        let epoch = aggregate(
            &mut types,
            vec![marker; 3],
            vec![0; 3],
            0,
            1,
            SemanticBackendReprV1::Memory { sized: true },
            None,
        );
        Some(aggregate(
            &mut types,
            vec![index, index, epoch, marker],
            vec![0, 8, 16, 16],
            16,
            8,
            SemanticBackendReprV1::scalar_pair(scalar, scalar),
            Some(SemanticExecutionRoleV29::Workgroup),
        ))
    } else {
        None
    };
    let (base, mut ownership) = match input {
        Input::Workgroup | Input::CapturedWorkgroup => (
            workgroup.unwrap(),
            SemanticSourceArgumentOwnershipV1::ByValue,
        ),
        Input::SharedWorkgroup | Input::CapturedWorkgroupReference => (
            reference(
                &mut types,
                workgroup.unwrap(),
                SemanticMutabilityV1::Immutable,
                false,
            ),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
        Input::RawContext => (
            reference(&mut types, CONTEXT, SemanticMutabilityV1::Mutable, true),
            SemanticSourceArgumentOwnershipV1::RawPointer,
        ),
        _ => (
            reference(&mut types, CONTEXT, SemanticMutabilityV1::Mutable, false),
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        ),
    };
    let selected = if matches!(
        input,
        Input::CapturedContext | Input::CapturedWorkgroup | Input::CapturedWorkgroupReference
    ) {
        let bytes = types[base.index() as usize].layout().size_bytes().unwrap();
        ownership = SemanticSourceArgumentOwnershipV1::ByValue;
        aggregate(
            &mut types,
            vec![base, U32],
            vec![0, bytes],
            (bytes + 4).next_multiple_of(8),
            8,
            SemanticBackendReprV1::Memory { sized: true },
            None,
        )
    } else {
        base
    };
    if wrong_ownership {
        ownership = SemanticSourceArgumentOwnershipV1::Unspecified;
    }
    let make_abi = |tag, kernel| {
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
            1,
            vec![SemanticAbiArgumentV1::source(value_abi(&types, selected))],
            ignored(UNIT),
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap()
    };
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        make_abi(203, true),
        vec![
            local(210, UNIT, SemanticLocalRoleV1::Return),
            local(211, selected, SemanticLocalRoleV1::Argument(0)),
            local(212, U32, SemanticLocalRoleV1::Temporary),
            local(213, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                220,
                vec![],
                call(1, vec![SemanticOperandV1::Move(place(1, selected))], 1),
            ),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"nominal_plan_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([204; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(205, false),
        vec![
            local(230, UNIT, SemanticLocalRoleV1::Return),
            local(231, selected, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(240, vec![], SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(HELPER),
        ],
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

#[test]
fn nominal_rust_carriers_do_not_become_physical_arguments() {
    for input in [
        Input::MutableContext,
        Input::Workgroup,
        Input::SharedWorkgroup,
        Input::CapturedContext,
        Input::CapturedWorkgroup,
        Input::CapturedWorkgroupReference,
    ] {
        let owner = nominal_owner(input, false);
        let plan = plan_case(owner, 300, false, 10_000_000, 10_000_000)
            .0
            .unwrap_or_else(|error| panic!("{input:?}: {error:?}"));
        let scalar = matches!(
            input,
            Input::CapturedContext | Input::CapturedWorkgroup | Input::CapturedWorkgroupReference
        );
        assert_eq!(
            plan.parameter_types,
            if scalar {
                vec![Type::Scalar(ScalarType::U32)]
            } else {
                vec![]
            }
        );
        assert_eq!(
            plan.parameter_values,
            if scalar { vec![ValueId(300)] } else { vec![] }
        );
        assert!(plan.result_types.is_empty());
        assert!(plan.parameter_local_bindings.is_empty());
        assert!(plan.ignored_parameter_bindings.is_empty());
        if scalar {
            assert_eq!(plan.call_arguments[0].source_argument, 0);
            assert_eq!(plan.call_arguments[0].tuple_field, None);
            assert_eq!(plan.call_arguments[0].component, Some(0));
        }
    }
}

#[test]
fn nominal_plan_rejects_raw_pointers_and_unspecified_ownership() {
    assert!(
        plan_case(
            nominal_owner(Input::RawContext, false),
            300,
            false,
            10_000_000,
            10_000_000
        )
        .0
        .is_err()
    );
    for input in [
        Input::MutableContext,
        Input::SharedWorkgroup,
        Input::CapturedContext,
        Input::Workgroup,
    ] {
        assert!(
            plan_case(
                nominal_owner(input, true),
                300,
                false,
                10_000_000,
                10_000_000
            )
            .0
            .is_err(),
            "{input:?}"
        );
    }
}

#[test]
fn nominal_borrow_abi_policy_does_not_enable_ordinary_helper_arguments() {
    for input in [
        Input::MutableContext,
        Input::CapturedContext,
        Input::CapturedWorkgroupReference,
    ] {
        let owner = nominal_owner(input, false);
        let semantic = owner.source_semantic();
        let helper = &semantic.functions()[HELPER.index() as usize];
        let arguments = semantic.logical_arguments_v1(HELPER).unwrap();
        assert!(
            helper_parameter_shape_v1(
                semantic.types(),
                helper,
                HELPER,
                arguments.adjusted_arguments().next().unwrap()
            )
            .is_err()
        );
    }
}
