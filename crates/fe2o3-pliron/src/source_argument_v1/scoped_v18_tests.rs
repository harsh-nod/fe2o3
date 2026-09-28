use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Signature, SliceType};
use std::cell::Cell;

#[path = "source_raw_helper_v18_tests.rs"]
mod raw_helper;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const NESTED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
        layout,
        shape,
    )
}

fn fixture_type_ids(arguments: &[SemanticTypeIdV1]) -> [Option<SemanticTypeIdV1>; 6] {
    // The constants index the fixture catalog, not every admitted request.
    // Keep only the root signature's transitive closure and assign dense IDs.
    let mut included = [true, false, false, false, false, false];
    for argument in arguments {
        let dependencies: &[SemanticTypeIdV1] = match *argument {
            UNIT => &[UNIT],
            U32 => &[U32],
            SLICE => &[U32, SLICE],
            REFERENCE => &[U32, SLICE, REFERENCE],
            PAIR => &[U32, PAIR],
            NESTED => &[U32, SLICE, REFERENCE, NESTED],
            _ => panic!("unknown fixture type"),
        };
        for ty in dependencies {
            included[ty.index() as usize] = true;
        }
    }
    let mut next = 0;
    included.map(|include| {
        include.then(|| {
            let id = SemanticTypeIdV1::from_index(next);
            next += 1;
            id
        })
    })
}

fn fixture_types(ids: &[Option<SemanticTypeIdV1>; 6]) -> Vec<SemanticTypeDeclV1> {
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let first = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let second = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let reference_properties = SemanticTypeAbiPropertiesV1::new(false, false)
        .with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        );
    let reference = declaration(
        7,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(first, second),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SLICE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(reference_properties);
    vec![
        declaration(
            1,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        declaration(
            3,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        declaration(
            5,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(4, 0),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(false),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice { element: U32 },
        ),
        reference,
        declaration(
            9,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                4,
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 4, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, UNIT, U32]).unwrap()),
        ),
        declaration(
            11,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(first, second),
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap()),
        )
        .with_rustc_abi_properties(reference_properties),
    ]
    .into_iter()
    .enumerate()
    // Only the tuple envelopes can move: their edges refer to the retained
    // UNIT/U32 or UNIT/U32/SLICE/REFERENCE prefix, whose IDs are unchanged.
    .filter_map(|(index, ty)| ids[index].map(|_| ty))
    .collect()
}

fn semantic(arguments: &[SemanticTypeIdV1]) -> AdmittedInertSemanticMirV1 {
    let ids = fixture_type_ids(arguments);
    let id = |ty: SemanticTypeIdV1| ids[ty.index() as usize].unwrap();
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let reference = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let value = |ty| {
        SemanticAbiValueV1::new(
            id(ty),
            match ty {
                UNIT => SemanticAbiPassModeV1::Ignore,
                REFERENCE | NESTED => SemanticAbiPassModeV1::Pair {
                    first: reference,
                    second: plain,
                },
                PAIR => SemanticAbiPassModeV1::Pair {
                    first: plain,
                    second: plain,
                },
                _ => SemanticAbiPassModeV1::Direct(plain),
            },
        )
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        arguments.len() as u32,
        arguments
            .iter()
            .copied()
            .map(|ty| SemanticAbiArgumentV1::source(value(ty)))
            .collect(),
        value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(
        arguments
            .iter()
            .map(|ty| {
                if *ty == REFERENCE {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::ByValue
                }
            })
            .collect(),
    )
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([40; 32]),
        UNIT,
        SemanticLocalRoleV1::Return,
        source,
    )];
    locals.extend(arguments.iter().enumerate().map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([41 + index as u8; 32]),
            id(*ty),
            SemanticLocalRoleV1::Argument(index as u32),
            source,
        )
    }));
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([50; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([55; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"scoped_v18_argument_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([56; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        fixture_types(&ids),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn argument_fixtures_retain_exact_root_closure_and_pointee_abi_refusals() {
    for arguments in [
        vec![],
        vec![UNIT],
        vec![U32],
        vec![REFERENCE],
        vec![PAIR, UNIT],
        vec![NESTED],
        vec![REFERENCE, REFERENCE],
        vec![PAIR, NESTED],
    ] {
        let source = semantic(&arguments);
        let ids = fixture_type_ids(&arguments);
        let retained: Vec<_> = ids
            .iter()
            .enumerate()
            .filter_map(|(catalog, id)| id.map(|id| (catalog, id)))
            .collect();
        assert_eq!(source.types().len(), retained.len());
        for (dense, (catalog, id)) in retained.into_iter().enumerate() {
            assert_eq!(id.index() as usize, dense);
            assert_eq!(
                source.types()[dense].identity(),
                SemanticTypeIdentityV1::from_sha256([1 + 2 * catalog as u8; 32]),
            );
        }
        assert_eq!(
            source.functions()[0].abi().source_input_types(),
            arguments
                .iter()
                .map(|ty| ids[ty.index() as usize].unwrap())
                .collect::<Vec<_>>(),
        );

        let mut types = source.types().to_vec();
        let unused = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(declaration(
            101,
            source.types()[0].layout().clone(),
            SemanticTypeShapeV1::Unit,
        ));
        assert_eq!(
            InertSemanticMirRequestV1::new(
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                    [250; 32]
                ),),
                types,
                vec![],
                vec![],
                vec![],
                source.functions().to_vec(),
                vec![ROOT],
            )
            .unwrap()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
            SemanticMirErrorV1::TypeOutsideRootClosure { ty: unused },
        );
    }

    let source = semantic(&[NESTED]);
    let mut types = source.types().to_vec();
    let nested = source.functions()[0].abi().source_input_types()[0].index() as usize;
    types[nested] = types[nested]
        .clone()
        .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(false, false));
    assert_eq!(
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types,
            vec![],
            vec![],
            vec![],
            source.functions().to_vec(),
            vec![ROOT],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi,
    );
}

fn entry(function: &Function) -> ArgumentEntryV18<'_> {
    ArgumentEntryV18 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        kernel_ir_function: &function.id,
        role: SemanticKirFunctionRoleV1::KernelEntry,
    }
}

fn direct(local: u32, value: u32) -> SemanticKirParameterBindingV1 {
    SemanticKirParameterBindingV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        semantic_local: SemanticLocalIdV1::from_index(local),
        kernel_ir_value: ValueId(value),
    }
}

fn slice(space: AddressSpace) -> Type {
    Type::Slice(SliceType::new(
        Type::Scalar(ScalarType::U32),
        space,
        AccessMode::ReadOnly,
    ))
}

fn target(parameters: Vec<Type>) -> Function {
    let values = (0..parameters.len() as u32).map(ValueId).collect();
    Function::kernel_entry(
        "scoped_v18_argument_fixture",
        Signature::new(parameters, vec![]),
        values,
        vec![],
    )
}

fn trace(rows: &[SemanticKirParameterBindingV1]) -> ArgumentTraceV1<'_> {
    ArgumentTraceV1 {
        direct: rows,
        components: &[],
        ignored: &[],
    }
}

#[test]
fn actual_scalar_and_shared_slice_reuse_existing_node_and_trace_types() {
    for (ty, actual, global) in [
        (U32, Type::Scalar(ScalarType::U32), false),
        (REFERENCE, slice(AddressSpace::Generic), false),
        (REFERENCE, slice(AddressSpace::Global), true),
    ] {
        let source = semantic(&[ty]);
        let target = target(vec![actual]);
        let rows = [direct(1, 0)];
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        let mut proposal = |ordinal, observed, _: &mut ArgumentBudgetV1<'_>| {
            assert_eq!((ordinal, observed), (0, ty));
            Ok(global)
        };
        with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            Some(&mut proposal),
            |view, budget| {
                let mut nodes = 0;
                view.visit_nodes_with_budget(
                    budget,
                    |node: ProductionArgumentNodeV1<'_>, budget| {
                        nodes += 1;
                        let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage()
                        else {
                            panic!("physical leaf");
                        };
                        assert_eq!(
                            parameter.trace(),
                            ProductionArgumentTraceV1::Direct(&rows[0])
                        );
                        assert_eq!(
                            view.physical(0, budget)?.unwrap().value(),
                            parameter.value()
                        );
                        Ok(())
                    },
                )?;
                assert_eq!(nodes, 1);
                assert!(view.reborrow().physical(1, budget)?.is_none());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), 17);
        assert!(!cleanup.refund_denied());
    }
}

#[test]
fn by_value_components_zero_fields_and_ignored_locals_are_not_dropped() {
    let source = semantic(&[PAIR, UNIT]);
    let target = target(vec![Type::Scalar(ScalarType::U32); 2]);
    let components = [0, 2].map(|field| SemanticKirParameterComponentBindingV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        semantic_local: SemanticLocalIdV1::from_index(1),
        semantic_component_type: U32,
        projection: vec![SemanticKirParameterProjectionV1::Field(field)].into_boxed_slice(),
        kernel_ir_value: ValueId(u32::from(field != 0)),
    });
    let ignored = [SemanticKirIgnoredParameterBindingV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        semantic_local: SemanticLocalIdV1::from_index(2),
        semantic_type: UNIT,
    }];
    let cleanup = CanonicalAnalysisCleanupV1::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    with_parameter_correspondence_v18(
        &source,
        entry(&target),
        &target,
        ArgumentTraceV1 {
            direct: &[],
            components: &components,
            ignored: &ignored,
        },
        &cleanup,
        &mut budget,
        None,
        |view, budget| {
            let (mut nodes, mut zero, mut physical) = (0, 0, 0);
            view.visit_nodes_with_budget(budget, |node, _| {
                nodes += 1;
                match node.coverage() {
                    ProductionArgumentCoverageV1::Zero => zero += 1,
                    ProductionArgumentCoverageV1::Parameter(_) => physical += 1,
                    _ => {}
                }
                Ok(())
            })?;
            assert_eq!((nodes, zero, physical), (5, 2, 2));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn shape_proposal_is_inert_and_actual_slot_order_is_replayed() {
    let source = semantic(&[REFERENCE, REFERENCE]);
    let rows = [direct(1, 0), direct(2, 1)];
    // A matching forged answer describes inert data, but creates no profile or proof.
    for actual in [
        vec![slice(AddressSpace::Global), slice(AddressSpace::Generic)],
        vec![slice(AddressSpace::Generic), slice(AddressSpace::Global)],
    ] {
        let target = target(actual);
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut next = 0;
        let mut alternating = |ordinal, ty, _: &mut ArgumentBudgetV1<'_>| {
            assert_eq!((ordinal, ty), (next, REFERENCE));
            next += 1;
            Ok(ordinal == 0)
        };
        let mut entered = false;
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            Some(&mut alternating),
            |_, _| {
                entered = true;
                Ok(())
            },
        );
        let matching = matches!(&target.signature.parameters[0], Type::Slice(ty) if ty.address_space == AddressSpace::Global);
        assert_eq!(result.is_ok(), matching);
        assert_eq!(entered, matching);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn shape_proposal_cannot_promote_scalar_nested_or_helper_arguments() {
    for ty in [U32, NESTED, REFERENCE] {
        let source = semantic(&[ty]);
        let target = target(vec![if ty == U32 {
            Type::Scalar(ScalarType::U32)
        } else {
            slice(AddressSpace::Global)
        }]);
        let mut association = entry(&target);
        if ty == REFERENCE {
            association.role = SemanticKirFunctionRoleV1::InternalHelper;
        }
        let rows = [direct(1, 0)];
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut forged = |_, _, _: &mut ArgumentBudgetV1<'_>| Ok(true);
        let mut entered = false;
        assert!(
            with_parameter_correspondence_v18(
                &source,
                association,
                &target,
                trace(&rows),
                &cleanup,
                &mut budget,
                Some(&mut forged),
                |_, _| {
                    entered = true;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!entered);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_function_target_trace_and_shape_mismatches_precede_callback() {
    for mutation in 0..9 {
        let source = semantic(&[U32]);
        let mut target = target(vec![Type::Scalar(ScalarType::U32)]);
        let mut rows = vec![direct(1, 0)];
        let foreign = FunctionId::new("foreign");
        match mutation {
            2 => rows[0].correspondence_owner = SemanticFunctionIdV1::from_index(1),
            3 => rows[0].semantic_function = SemanticFunctionIdV1::from_index(1),
            4 => rows[0].semantic_local = SemanticLocalIdV1::from_index(0),
            5 => rows[0].kernel_ir_value = ValueId(1),
            6 => target.signature.parameters[0] = Type::Scalar(ScalarType::U64),
            7 => rows.clear(),
            8 => rows.push(rows[0]),
            _ => {}
        }
        let mut association = entry(&target);
        if mutation == 0 {
            association.semantic_function = SemanticFunctionIdV1::from_index(1);
        }
        if mutation == 1 {
            association.kernel_ir_function = &foreign;
        }
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut entered = false;
        assert!(
            with_parameter_correspondence_v18(
                &source,
                association,
                &target,
                trace(&rows),
                &cleanup,
                &mut budget,
                None,
                |_, _| {
                    entered = true;
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(!entered, "mutation {mutation}");
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn absent_or_false_profile_cannot_match_an_unchanged_global_source_slice() {
    let source = semantic(&[REFERENCE]);
    let target = target(vec![slice(AddressSpace::Global)]);
    let rows = [direct(1, 0)];
    assert_eq!(
        lower_parameter_type(source.types(), source.callables(), REFERENCE).unwrap(),
        slice(AddressSpace::Global),
        "the legacy ABI mapping remains unchanged",
    );
    assert_eq!(
        source_parameter_type_v18(source.types(), source.callables(), REFERENCE).unwrap(),
        slice(AddressSpace::Generic),
    );
    for supplied in [false, true] {
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        let proposals = Cell::new(0);
        let visits = Cell::new(0);
        let mut false_proposal = |_, _, _: &mut ArgumentBudgetV1<'_>| {
            proposals.set(proposals.get() + 1);
            Ok(false)
        };
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            if supplied {
                Some(&mut false_proposal)
            } else {
                None
            },
            |_, _| {
                visits.set(visits.get() + 1);
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(proposals.get(), usize::from(supplied));
        assert_eq!(visits.get(), 0);
        assert_eq!(budget.storage(), 17);
        assert!(!cleanup.refund_denied());
        assert_eq!(target.signature.parameters, [slice(AddressSpace::Global)]);
        assert_eq!(rows[0].kernel_ir_value, ValueId(0));
    }
}

#[test]
fn source_helper_shape_preserves_the_selected_leaf_policy_and_legacy_shape() {
    let source = semantic(&[NESTED]);
    let function = &source.functions()[0];
    let arguments = source.logical_arguments_v1(ROOT).unwrap();
    for policy in [
        ParameterLeafPolicyV1::SharedSliceLeaves,
        ParameterLeafPolicyV1::ExecutionAbiWords,
    ] {
        let mapped = arguments.adjusted_arguments().next().unwrap();
        let (legacy_shared, legacy) =
            helper_parameter_shape_with_policy_v1(source.types(), function, ROOT, mapped, policy)
                .unwrap();
        let mapped = arguments.adjusted_arguments().next().unwrap();
        let (source_shared, source_rows) = source_helper_parameter_shape_with_policy_v18(
            source.types(),
            function,
            ROOT,
            mapped,
            policy,
        )
        .unwrap();
        assert_eq!(source_shared, legacy_shared);
        assert_eq!(legacy.len(), 1);
        assert_eq!(source_rows.len(), 1);
        assert_eq!(source_rows[0].0, legacy[0].0);
        assert_eq!(source_rows[0].1, REFERENCE);
        assert_eq!(legacy[0].2, slice(AddressSpace::Global));
        assert_eq!(source_rows[0].2, slice(AddressSpace::Generic));
    }
    for source_shape in [false, true] {
        let mapped = arguments.adjusted_arguments().next().unwrap();
        let result = if source_shape {
            source_helper_parameter_shape_with_policy_v18(
                source.types(),
                function,
                ROOT,
                mapped,
                ParameterLeafPolicyV1::PointerFree,
            )
        } else {
            helper_parameter_shape_with_policy_v1(
                source.types(),
                function,
                ROOT,
                mapped,
                ParameterLeafPolicyV1::PointerFree,
            )
        };
        assert!(matches!(
            result,
            Err(ProductionSourceArgumentErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "embedded pointer kernel arguments have no owned region binding",
            })
        ));
    }
}

#[test]
fn foreign_slot_ledger_and_live_floor_loss_link_denial_without_refund() {
    for mutation in 0..3 {
        let source = semantic(&[U32]);
        let target = target(vec![Type::Scalar(ScalarType::U32)]);
        let rows = [direct(1, 0)];
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        let mut work = Work::new(usize::MAX);
        let mut foreign_work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        budget.reserve_storage(19).unwrap();
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                if mutation == 0 {
                    foreign.reserve_storage(budget.storage())?;
                    assert!(view.physical(0, &mut foreign).is_err());
                } else if mutation == 1 {
                    foreign.reserve_storage(budget.storage())?;
                    std::mem::swap(budget, &mut foreign);
                    assert!(view.physical(0, budget).is_err());
                } else {
                    budget.release_storage(budget.storage())?;
                    assert!(view.physical(0, budget).is_err());
                }
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(
                ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert!(parent.get());
        assert!(cleanup.refund_denied());
        if mutation == 2 {
            assert_eq!(budget.storage(), 0);
        } else {
            assert!(budget.storage() > 19);
        }
    }
}

#[test]
fn selected_errors_raw_payloads_and_sticky_parent_denial_are_preserved() {
    for panic in [false, true] {
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(13).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            argument_attempt_v18(
                &cleanup,
                &mut budget,
                13,
                |budget| -> Result<(), ProductionSourceArgumentErrorV1> {
                    budget.release_storage(budget.storage())?;
                    if panic {
                        std::panic::resume_unwind(Box::new(73_u32));
                    }
                    Err(ProductionSourceArgumentErrorV1::Visitor)
                },
            )
        }));
        if panic {
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 73);
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ProductionSourceArgumentErrorV1::Visitor)
            ));
        }
        assert_eq!(budget.storage(), 0);
        assert!(parent.get());
        parent.set(false);
        assert!(cleanup.refund_denied());
        let mut entered = false;
        assert!(
            argument_attempt_v18(&cleanup, &mut budget, 0, |_| {
                entered = true;
                Ok::<_, ProductionSourceArgumentErrorV1>(())
            })
            .is_err()
        );
        assert!(!entered);
    }
}

#[test]
fn exact_and_one_short_attempt_storage_preflight() {
    fn run(limit: usize) -> (bool, usize, usize) {
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = with_argument_scratch_v18(&cleanup, &mut budget, |_| {
            Ok::<_, ProductionSourceArgumentErrorV1>(())
        });
        (result.is_ok(), budget.peak_storage(), budget.storage())
    }
    let (_, peak, floor) = run(usize::MAX);
    assert_eq!(floor, 0);
    assert_eq!(run(peak), (true, peak, 0));
    assert!(!run(peak - 1).0);
    assert_eq!(run(peak - 1).2, 0);
}

#[repr(align(4096))]
struct LargeVisitorCapture([u8; 8192]);

impl LargeVisitorCapture {
    fn observe(&self) {
        std::hint::black_box(self);
    }
}

#[test]
fn owned_large_aligned_visitor_is_charged_at_exact_and_one_short_peak() {
    fn run(limit: usize, large: bool) -> (bool, usize, usize, usize, bool) {
        let source = semantic(&[U32]);
        let target = target(vec![Type::Scalar(ScalarType::U32)]);
        let rows = [direct(1, 0)];
        let cleanup = CanonicalAnalysisCleanupV1::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let invoked = Cell::new(0);
        let entered = Cell::new(false);
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                entered.set(true);
                if large {
                    let capture = LargeVisitorCapture([7; 8192]);
                    let invoked = &invoked;
                    view.visit_nodes_with_budget(budget, move |_, _| {
                        capture.observe();
                        invoked.set(invoked.get() + 1);
                        Ok(())
                    })
                } else {
                    view.visit_nodes_with_budget(budget, |_, _| {
                        invoked.set(invoked.get() + 1);
                        Ok(())
                    })
                }
            },
        );
        if result.is_err() {
            assert!(matches!(
                result,
                Err(
                    ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            assert!(budget.failed_storage().is_some());
        }
        assert!(!cleanup.refund_denied());
        (
            result.is_ok(),
            budget.peak_storage(),
            budget.storage(),
            invoked.get(),
            entered.get(),
        )
    }
    let (ok, peak, floor, invoked, entered) = run(usize::MAX, true);
    assert!(ok);
    assert_eq!(floor, 0);
    assert!(entered);
    assert_eq!(invoked, 1);
    assert!(peak >= run(usize::MAX, false).1 + std::mem::size_of::<LargeVisitorCapture>());
    assert_eq!(run(peak, true), (true, peak, 0, 1, true));
    let short = run(peak - 1, true);
    assert!(!short.0);
    assert_eq!(short.2, 0);
    assert_eq!(
        short.3, 0,
        "the complete visitor frame refuses before invocation"
    );
    assert!(
        short.4,
        "the refusal belongs to the owned visitor, not constructor entry"
    );
}

#[test]
fn rust_call_helper_keeps_source_tuple_envelope_and_adjusted_zero_field() {
    let root = semantic(&[PAIR]);
    let pair = root.functions()[0].abi().source_input_types()[0];
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == UNIT {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(plain)
            },
        )
    };
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([70; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        0,
        vec![pair],
        UNIT,
        [U32, UNIT, U32]
            .into_iter()
            .enumerate()
            .map(|(field, ty)| {
                SemanticAbiArgumentV1::rust_call_tuple_field(field as u32, value(ty))
            })
            .collect(),
        value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([71; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([72; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([73; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([74; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([75; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([76; 32]),
                UNIT,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([77; 32]),
                pair,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([78; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let root_types = root.types().to_vec();
    let original_root = &root.functions()[0];
    let call = SemanticDirectCallV1::new(
        SemanticFunctionIdV1::from_index(1),
        vec![SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], pair).unwrap(),
        )],
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], UNIT).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root_with_call = SemanticFunctionDeclV1::new(
        original_root.identity(),
        original_root.role(),
        original_root.item_definition_identity(),
        original_root.monomorphization_identity(),
        original_root.generic_type_arguments_identity(),
        original_root.const_generic_arguments_identity(),
        original_root.source(),
        original_root.abi().clone(),
        original_root.locals().to_vec(),
        original_root.entry(),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([54; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            original_root.blocks()[0].clone(),
        ],
    )
    .unwrap()
    .with_kernel_entry(original_root.kernel_entry().unwrap().clone());
    let request = |root| {
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            root_types.clone(),
            vec![],
            vec![],
            vec![],
            vec![root, helper.clone()],
            vec![ROOT],
        )
        .unwrap()
    };
    assert_eq!(
        request(original_root.clone())
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::FunctionOutsideRootClosure {
            function: SemanticFunctionIdV1::from_index(1),
        },
    );
    let semantic = request(root_with_call)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let helper = SemanticFunctionIdV1::from_index(1);
    let target = Function::internal_helper(
        "rust_call_argument_fixture",
        Signature::new(vec![Type::Scalar(ScalarType::U32); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![],
    );
    let rows = [0, 2].map(|field| SemanticKirParameterComponentBindingV1 {
        correspondence_owner: ROOT,
        semantic_function: helper,
        semantic_local: SemanticLocalIdV1::from_index(1),
        semantic_component_type: U32,
        projection: vec![SemanticKirParameterProjectionV1::Field(field)].into_boxed_slice(),
        kernel_ir_value: ValueId(u32::from(field != 0)),
    });
    let cleanup = CanonicalAnalysisCleanupV1::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    with_parameter_correspondence_v18(
        &semantic,
        ArgumentEntryV18 {
            correspondence_owner: ROOT,
            semantic_function: helper,
            kernel_ir_function: &target.id,
            role: SemanticKirFunctionRoleV1::InternalHelper,
        },
        &target,
        ArgumentTraceV1 {
            direct: &[],
            components: &rows,
            ignored: &[],
        },
        &cleanup,
        &mut budget,
        None,
        |view, budget| {
            let (mut envelopes, mut physical, mut zero) = (0, 0, 0);
            view.visit_nodes_with_budget(budget, |node, _| {
                assert_eq!(node.source_argument(), 0);
                if node.adjusted_argument().is_none() {
                    envelopes += 1;
                    assert_eq!(node.semantic_type(), pair);
                    assert!(node.source_path().is_empty());
                }
                match node.coverage() {
                    ProductionArgumentCoverageV1::Parameter(parameter) => {
                        physical += 1;
                        assert!(matches!(
                            parameter.trace(),
                            ProductionArgumentTraceV1::Component(_)
                        ));
                    }
                    ProductionArgumentCoverageV1::Zero => {
                        zero += 1;
                        assert_eq!(node.adjusted_argument(), Some(1));
                        assert_eq!(node.semantic_type(), UNIT);
                    }
                    _ => {}
                }
                Ok(())
            })?;
            assert_eq!((envelopes, physical, zero), (1, 2, 1));
            assert_eq!(view.physical(1, budget)?.unwrap().value(), ValueId(1));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn actual_shape_callback_preserves_selected_outcome_on_ledger_floor_and_linked_denial() {
    fn case<'work>(
        fault: u8,
        disposition: u8,
        budget: &mut ArgumentBudgetV1<'work>,
        foreign: &mut ArgumentBudgetV1<'work>,
    ) {
        let source = semantic(&[REFERENCE]);
        let target = target(vec![slice(AddressSpace::Global)]);
        let rows = [direct(1, 0)];
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        budget.reserve_storage(31).unwrap();
        foreign.reserve_storage(1_000_000).unwrap();
        foreign.charge_work(7).unwrap();
        let called = Cell::new(0);
        let entered = Cell::new(false);
        let retained = Cell::new(0);
        let accepted_work = Cell::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut proposal = |ordinal, ty, budget: &mut ArgumentBudgetV1<'work>| {
                called.set(called.get() + 1);
                assert_eq!((ordinal, ty), (0, REFERENCE));
                retained.set(budget.storage());
                accepted_work.set(budget.work());
                match fault {
                    0 => std::mem::swap(budget, foreign),
                    1 => {
                        budget.release_storage(budget.storage())?;
                    }
                    _ => cleanup.deny_refund(),
                }
                match disposition {
                    0 => Ok(true),
                    1 => Err(ProductionSourceArgumentErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "selected shape proposal error",
                    }),
                    _ => std::panic::resume_unwind(Box::new(1732_u32)),
                }
            };
            with_parameter_correspondence_v18(
                &source,
                entry(&target),
                &target,
                trace(&rows),
                &cleanup,
                budget,
                Some(&mut proposal),
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            )
        }));
        assert_eq!(called.get(), 1);
        assert!(!entered.get());
        match disposition {
            0 => assert!(matches!(
                result.unwrap(),
                Err(
                    ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            )),
            1 => assert!(matches!(
                result.unwrap(),
                Err(ProductionSourceArgumentErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "selected shape proposal error",
                })
            )),
            _ => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 1732),
        }
        assert!(parent.get());
        assert!(cleanup.refund_denied());
        if fault != 0 {
            assert_eq!(
                budget.work(),
                accepted_work.get(),
                "no checker work follows the observed shape-callback fault"
            );
        }
        match fault {
            0 => {
                assert_eq!((budget.storage(), budget.work()), (1_000_000, 7));
                assert_eq!(
                    (foreign.storage(), foreign.work()),
                    (retained.get(), accepted_work.get())
                );
                std::mem::swap(budget, foreign);
            }
            1 => assert_eq!(budget.storage(), 0),
            _ => assert_eq!(budget.storage(), retained.get()),
        }
    }
    for fault in 0..3 {
        for disposition in 0..3 {
            let mut work = Work::new(usize::MAX);
            let mut foreign_work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
            case(fault, disposition, &mut budget, &mut foreign);
        }
    }
}

#[test]
fn actual_shape_callback_cannot_suppress_nested_floor_refusal() {
    let source = semantic(&[REFERENCE]);
    let target = target(vec![slice(AddressSpace::Global)]);
    let rows = [direct(1, 0)];
    let parent = Cell::new(false);
    let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let called = Cell::new(0);
    let entered = Cell::new(false);
    let retained = Cell::new(0);
    let mut proposal = |_, _, budget: &mut ArgumentBudgetV1<'_>| {
        called.set(called.get() + 1);
        let floor = budget.storage();
        let refused = argument_attempt_v18(&cleanup, budget, floor, |budget| {
            budget.release_storage(1)?;
            Ok::<_, ProductionSourceArgumentErrorV1>(())
        });
        assert!(matches!(
            refused,
            Err(
                ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        budget.reserve_storage(1)?;
        assert!(
            cleanup.refund_denied(),
            "restoring the missing byte cannot clear observed denial"
        );
        retained.set(budget.storage());
        Ok(true)
    };
    let result = with_parameter_correspondence_v18(
        &source,
        entry(&target),
        &target,
        trace(&rows),
        &cleanup,
        &mut budget,
        Some(&mut proposal),
        |_, _| {
            entered.set(true);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(
            ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
    assert_eq!(called.get(), 1);
    assert!(!entered.get());
    assert!(parent.get());
    assert!(cleanup.refund_denied());
    assert_eq!(budget.storage(), retained.get());
}

struct VisitorDisposal<'a> {
    cleanup: &'a CanonicalAnalysisCleanupV1<'a>,
    dropped: &'a Cell<bool>,
    deny: bool,
    panic: bool,
}

impl VisitorDisposal<'_> {
    fn observe(&self) {
        std::hint::black_box(self);
    }
}

impl Drop for VisitorDisposal<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
        if self.deny {
            self.cleanup.deny_refund();
        }
        if self.panic {
            std::panic::resume_unwind(Box::new(91_u32));
        }
    }
}

#[test]
fn owned_visitor_disposal_precedes_settlement_and_preserves_selected_outcomes() {
    // A destructor that itself panics during unwinding has ordinary Rust
    // double-panic behavior; the preservation cases use a non-panicking Drop.
    for mode in 0..4 {
        let source = semantic(&[U32]);
        let target = target(vec![Type::Scalar(ScalarType::U32)]);
        let rows = [direct(1, 0)];
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        let dropped = Cell::new(false);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_parameter_correspondence_v18(
                &source,
                entry(&target),
                &target,
                trace(&rows),
                &cleanup,
                &mut budget,
                None,
                |view, budget| {
                    let disposal = VisitorDisposal {
                        cleanup: &cleanup,
                        dropped: &dropped,
                        deny: mode != 3,
                        panic: mode == 3,
                    };
                    view.visit_nodes_with_budget(budget, move |_, _| {
                        disposal.observe();
                        match mode {
                            1 => Err(ProductionSourceArgumentErrorV1::Visitor),
                            2 => std::panic::resume_unwind(Box::new(73_u32)),
                            _ => Ok(()),
                        }
                    })
                },
            )
        }));
        assert!(dropped.get());
        match mode {
            0 => assert!(matches!(
                result.unwrap(),
                Err(
                    ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            )),
            1 => assert!(matches!(
                result.unwrap(),
                Err(ProductionSourceArgumentErrorV1::Visitor)
            )),
            2 => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 73),
            3 => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 91),
            _ => unreachable!(),
        }
        assert_eq!(parent.get(), mode != 3);
        assert_eq!(cleanup.refund_denied(), mode != 3);
        if mode == 3 {
            assert_eq!(budget.storage(), 23);
        } else {
            assert!(budget.storage() > 23);
        }
    }
}

include!("scoped_v18_locator_tests.rs");

#[test]
fn inert_custody_check_preserves_live_owner_floor_and_all_meter_counts() {
    let source = semantic(&[U32]);
    let target = target(vec![Type::Scalar(ScalarType::U32)]);
    let rows = [direct(1, 0)];
    let cleanup = CanonicalAnalysisCleanupV1::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    with_parameter_correspondence_v18(
        &source,
        entry(&target),
        &target,
        trace(&rows),
        &cleanup,
        &mut budget,
        None,
        |view, budget| {
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            for _ in 0..3 {
                view.check_custody(budget)?;
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                before
            );
            assert_eq!(
                view.physical(0, budget)?.unwrap().value(),
                target.body.as_ref().unwrap().parameters[0]
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 19);
    assert!(!cleanup.refund_denied());
}

#[test]
fn inert_custody_check_refuses_foreign_slot_ledger_and_lost_floor_without_charge() {
    for mutation in 0..3 {
        let source = semantic(&[U32]);
        let target = target(vec![Type::Scalar(ScalarType::U32)]);
        let rows = [direct(1, 0)];
        let parent = Cell::new(false);
        let cleanup = CanonicalAnalysisCleanupV1::linked(&parent);
        let mut work = Work::new(usize::MAX);
        let mut foreign_work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        budget.reserve_storage(19).unwrap();
        let result = with_parameter_correspondence_v18(
            &source,
            entry(&target),
            &target,
            trace(&rows),
            &cleanup,
            &mut budget,
            None,
            |view, budget| {
                if mutation == 0 {
                    foreign.reserve_storage(budget.storage())?;
                    let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                    assert!(matches!(
                        view.check_custody(&foreign),
                        Err(
                            ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(
                        (foreign.work(), foreign.storage(), foreign.peak_storage()),
                        before
                    );
                } else {
                    if mutation == 1 {
                        foreign.reserve_storage(budget.storage())?;
                        std::mem::swap(budget, &mut foreign);
                    } else {
                        budget.release_storage(budget.storage())?;
                    }
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    assert!(matches!(
                        view.check_custody(budget),
                        Err(
                            ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        before
                    );
                }
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(
                ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert!(parent.get() && cleanup.refund_denied());
        if mutation == 2 {
            assert_eq!(budget.storage(), 0);
        } else {
            assert!(budget.storage() > 19);
        }
    }
}
