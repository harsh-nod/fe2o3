use super::*;

fn return_owner(shape: Shape, repeated: bool) -> ProductionSemanticSsaOwnerV1 {
    let template = owner(Leaf::Shared, shape);
    let semantic = template.source_semantic();
    let original = &semantic.functions()[0];
    let types = semantic.types().to_vec();
    let ty = original.abi().source_input_types()[0];
    let mut locals = original.locals().to_vec();
    let destination = locals.len() as u32;
    locals.push(local(240, ty, SemanticLocalRoleV1::Temporary));
    let call = |source, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Copy(place(source, ty))],
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let mut blocks = vec![block(211, vec![], call(1, 1))];
    if repeated {
        blocks.push(block(212, vec![], call(destination, 2)));
    }
    blocks.push(block(213, vec![], SemanticTerminatorKindV1::Return));
    let root = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        original.abi().clone(),
        locals,
        blocks,
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([231; 32]),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value_abi(&types, ty))],
        value_abi(&types, ty),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let helper = function(
        230,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        vec![
            local(232, ty, SemanticLocalRoleV1::Return),
            local(233, ty, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            234,
            vec![assign(
                place(0, ty),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, ty))),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    super::super::super::fixtures::build(
        types,
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
    )
}

#[test]
fn original_shared_slice_return_shape_walk_preserves_nominal_policy_and_exact_limits() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        let shared_owner = owner(Leaf::Shared, shape);
        let source = shared_owner.source_semantic();
        let ty = source.functions()[0].abi().source_input_types()[0];
        let (nodes, leaves) = match shape {
            Shape::Field => (2, 1),
            Shape::Array => (2, 2),
            Shape::Nested => (4, 3),
        };
        for shared in [false, true] {
            for short in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(nodes - usize::from(short));
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result = if shared {
                    execution_cfg_return_transport_count_v29(source.types(), ty, &mut budget)
                } else {
                    execution_cfg_nominal_count_v29(source.types(), ty, &mut budget)
                };
                if short {
                    assert!(
                        matches!(
                            result,
                            Err(
                                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Work(_)
                                )
                            )
                        ),
                        "{shape:?}, shared={shared}: {result:?}"
                    );
                    assert_eq!(budget.work(), nodes - 1);
                } else {
                    assert_eq!(result.unwrap(), if shared { leaves } else { 0 });
                    assert_eq!(budget.work(), nodes);
                }
                assert_eq!(budget.storage(), 0);
            }
        }
        for leaf in [
            Leaf::Raw,
            Leaf::Mutable,
            Leaf::Thin,
            Leaf::Global,
            Leaf::Local32,
        ] {
            let owner = owner(leaf, shape);
            let source = owner.source_semantic();
            let ty = source.functions()[0].abi().source_input_types()[0];
            let mut work = CanonicalKernelIrWorkBudgetV1::new(nodes);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert_eq!(
                execution_cfg_return_transport_count_v29(source.types(), ty, &mut budget).unwrap(),
                0,
                "fully admitted {leaf:?} is not an unprofiled shared-slice return"
            );
            assert_eq!((budget.work(), budget.storage()), (nodes, 0));
        }
    }
}

#[test]
fn shared_slice_return_shape_walk_is_bounded_on_missing_recursive_and_oversized_types() {
    let owner = owner(Leaf::Shared, Shape::Field);
    let source = owner.source_semantic();
    let ty = source.functions()[0].abi().source_input_types()[0];
    let original = &source.types()[ty.index() as usize];
    for mode in 0..3 {
        let mut types = source.types().to_vec();
        let mut queried = ty;
        let expected_work = match mode {
            0 => {
                queried = SemanticTypeIdV1::from_index(types.len() as u32);
                1
            }
            1 => {
                // This deliberately unadmitted recursive declaration exercises
                // only the bounded shape query, never source transport.
                types[ty.index() as usize] = SemanticTypeDeclV1::new(
                    original.identity(),
                    original.layout_identity(),
                    original.layout().clone(),
                    SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty]).unwrap()),
                );
                MAX_SSA_VALUE_COMPONENTS_V1 + 1
            }
            2 => {
                let SemanticTypeShapeV1::Aggregate(fields) = original.shape() else {
                    unreachable!()
                };
                types[ty.index() as usize] = SemanticTypeDeclV1::new(
                    original.identity(),
                    original.layout_identity(),
                    original.layout().clone(),
                    SemanticTypeShapeV1::Array {
                        element: fields.fields()[0],
                        length: (MAX_SSA_VALUE_COMPONENTS_V1 + 1) as u64,
                    },
                );
                2
            }
            _ => unreachable!(),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(
            execution_cfg_return_transport_count_v29(&types, queried, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "execution CFG transport differs from its captured SSA state",
                ..
            })
        ));
        assert_eq!((budget.work(), budget.storage()), (expected_work, 0));
    }
}

#[test]
fn original_shared_slice_aggregate_returns_join_each_actual_instance_and_exact_original_abi() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        let mut completed = false;
        with_selected_pointer_test_plan_v29(return_owner(shape, true), |plan, budget| {
            let source = plan.instances.owner().source_semantic();
            let calls = plan.instances.calls(plan.root).unwrap();
            assert_eq!(calls.len(), 2);
            let mut previous = None;
            for call in calls {
                let child = call.child().unwrap();
                assert_ne!(Some(child), previous);
                previous = Some(child);
                let declaration = plan.instances.instance(child).unwrap().declaration();
                assert_eq!(declaration.abi().source_argument_ownership(), [SemanticSourceArgumentOwnershipV1::ByValue]);
                assert_eq!(declaration.abi().source_output_type(), declaration.abi().source_input_types()[0]);
                let returned = source_reference_return_types_v29(plan, child, budget)?.unwrap();
                assert_eq!(returned.len(), expected_paths(shape).len());
                for ty in &returned {
                    assert!(matches!(ty, Type::Slice(slice) if slice.address_space == AddressSpace::Generic
                        && slice.access == AccessMode::ReadOnly));
                }
                // Neither the return layout nor copied ABI metadata grants the
                // standalone pointer-free route an owned region.
                assert!(matches!(source_abi_components_v18(source.types(), declaration,
                    declaration.abi().return_value(), ParameterLeafPolicyV1::PointerFree),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "embedded pointer kernel arguments have no owned region binding", ..
                    })));
            }
            completed = true;
            Ok(())
        }).unwrap();
        assert!(completed);
    }
}

#[test]
fn whole_shared_slice_field_array_and_nested_returns_complete_immutable_source_replay() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        for repeated in [false, true] {
            for preexisting in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                budget.reserve_storage(MODULE_FLOOR).unwrap();
                let mut completed = false;
                with_pending_api_owner_v18(
                    ModuleFixture::Ordinary,
                    preexisting,
                    &mut budget,
                    || return_owner(shape, repeated),
                    |owner, launch, input, _, budget| {
                        let original_identity = *owner.source_semantic_sha256();
                        let fixture =
                            kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
                        let roots = fixture.roots();
                        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                            owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                            ProductionSemanticKirLimitsV1::default(), budget).unwrap();
                        prepared.with_checked_source_v18(budget, |source, budget| {
                            assert_eq!(source.source_ssa(budget)?.source_semantic_sha256(), &original_identity);
                            let canonical = source.canonical(budget)?;
                            let root = &canonical.module().functions[source.root(0, budget)?.1];
                            let count = expected_paths(shape).len();
                            let helpers = if repeated { 2 } else { 1 };
                            assert_eq!(root.signature.parameters.len(), count);
                            assert_eq!(source.instance_count(0, budget)?, helpers + 1);
                            for instance in 1..=helpers {
                                assert!(source.instance_active(0, instance, budget)?);
                                assert_eq!(source.instance(0, instance, budget)?,
                                    (SemanticFunctionIdV1::from_index(1), Some((0, SemanticBlockIdV1::from_index((instance - 1) as u32)))));
                                let mut mapped = std::collections::BTreeSet::new();
                                for ordinal in 0..source.span_count(0, budget)? {
                                    for segment in 0..2 {
                                        if let Some((owner, block, _, _)) = source.span_segment(0, ordinal, segment, budget)?
                                            && owner == instance { mapped.insert(block); }
                                    }
                                }
                                assert_eq!(mapped.len(), 1);
                                let body = root.body.as_ref().unwrap();
                                let block = body.blocks.iter().find(|row| mapped.contains(&row.id)).unwrap();
                                assert!(block.parameters.is_empty());
                                let mut entries = body.blocks.iter().filter(|row|
                                    matches!(&row.terminator, Some(Terminator::Branch { target, arguments })
                                        if *target == block.id && arguments.is_empty()));
                                let entry = entries.next().expect("unique scoped invocation preheader");
                                assert!(entries.next().is_none());
                                assert_eq!(entry.parameters.len(), count);
                                for parameter in &entry.parameters {
                                    assert!(matches!(&parameter.ty, Type::Slice(slice)
                                        if slice.address_space == AddressSpace::Generic && slice.access == AccessMode::ReadOnly));
                                }
                                let Some(Terminator::Branch { target, arguments }) = &block.terminator else { panic!("expanded original return edge") };
                                assert_eq!(arguments, &entry.parameters.iter().map(|value| value.id).collect::<Vec<_>>());
                                let continuation = body.blocks.iter().find(|row| row.id == *target).unwrap();
                                assert_eq!(continuation.parameters.len(), count);
                                for (result, input) in continuation.parameters.iter().zip(&entry.parameters) {
                                    assert_eq!(result.ty, input.ty);
                                }
                            }
                            completed = true;
                            Ok(())
                        }).unwrap();
                    },
                );
                assert!(
                    completed,
                    "{shape:?}, repeated={repeated}, preexisting={preexisting}"
                );
                assert_eq!(budget.storage(), MODULE_FLOOR);
                budget.release_storage(MODULE_FLOOR).unwrap();
            }
        }
    }
}

#[test]
fn original_shared_slice_return_reconstruction_rejects_missing_wrong_space_access_and_element() {
    for shape in [Shape::Field, Shape::Array, Shape::Nested] {
        for fault in [None, Some(0), Some(1), Some(2), Some(3)] {
            let mut completed = false;
            let result =
                with_selected_pointer_test_plan_v29(return_owner(shape, false), |plan, budget| {
                    let child = plan.instances.calls(plan.root).unwrap()[0].child().unwrap();
                    let node = plan.returns[child.index()].unwrap();
                    let types = source_reference_return_types_v29(plan, child, budget)?.unwrap();
                    let emission = SourceReferenceEmissionV29::new(plan, budget)?;
                    // Isolate the existing result-shape boundary. These test IDs
                    // are never archived or published as an admitted physical body;
                    // the separate full consuming test exercises real call results.
                    let original: Vec<_> = types
                        .into_iter()
                        .enumerate()
                        .map(|(index, ty)| ValueDef::new(ValueId(index as u32 + 800), ty))
                        .collect();
                    let mut values = original.iter();
                    let binding = source_reference_rebuild_node_v29(
                        &emission,
                        node,
                        true,
                        &mut [].iter(),
                        &mut values,
                        &mut 0,
                        budget,
                    )?;
                    assert!(values.next().is_none());
                    source_reference_call_shape_v29(&emission, node, &binding, budget)?;
                    drop(binding);
                    let Some(fault) = fault else {
                        completed = true;
                        return Ok(());
                    };
                    let mut wrong = original.clone();
                    if fault == 0 {
                        wrong.pop();
                    } else {
                        let Type::Slice(slice) = &mut wrong[0].ty else {
                            unreachable!()
                        };
                        match fault {
                            1 => slice.address_space = AddressSpace::Global,
                            2 => slice.access = AccessMode::ReadWrite,
                            3 => slice.element = Box::new(Type::Scalar(ScalarType::U64)),
                            _ => unreachable!(),
                        }
                    }
                    let error = source_reference_rebuild_node_v29(
                        &emission,
                        node,
                        true,
                        &mut [].iter(),
                        &mut wrong.iter(),
                        &mut 0,
                        budget,
                    )
                    .unwrap_err();
                    assert!(matches!(
                        error,
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "execution CFG transport differs from its captured SSA state",
                            ..
                        }
                    ));
                    completed = true;
                    Err(error)
                });
            assert!(
                completed,
                "all result-boundary checks must execute: {shape:?}, fault={fault:?}: {result:?}"
            );
            if fault.is_none() {
                result.unwrap();
            } else {
                assert!(
                    matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "execution CFG transport differs from its captured SSA state",
                            ..
                        })
                    ),
                    "the exact reconstruction refusal must propagate: {result:?}"
                );
            }
        }
    }
}
