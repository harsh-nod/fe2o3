#[derive(Clone, Copy, Debug)]
enum DescriptorFlowV29 {
    Root,
    Branch,
    Loop,
    RepeatedHelper,
    Mixed,
}

#[path = "production_source_by_value_shared_slice_v29_tests.rs"]
mod by_value_shared_slice_v29;

fn descriptor_flow_owner_v29(flow: DescriptorFlowV29) -> ProductionSemanticSsaOwnerV1 {
    let template = descriptor_source_owner(DescriptorCase::READ);
    let semantic = template.source_semantic();
    let original = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(
        types
            .iter()
            .position(|ty| {
                matches!(
                    ty.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                )
            })
            .unwrap() as u32,
    );
    let slice = original.abi().source_input_types()[0];
    let word = original.abi().source_input_types()[2];
    let mixed = matches!(flow, DescriptorFlowV29::Mixed);
    let opaque = mixed.then(|| {
        aggregate(
            &mut types,
            vec![slice],
            vec![0],
            16,
            8,
            SemanticBackendReprV1::memory(true),
            None,
        )
    });
    let mut arguments = original.abi().arguments().to_vec();
    let mut ownership = original.abi().source_argument_ownership().to_vec();
    let mut locals = vec![
        local(202, UNIT, SemanticLocalRoleV1::Return),
        local(203, slice, SemanticLocalRoleV1::Argument(0)),
        local(204, slice, SemanticLocalRoleV1::Argument(1)),
        local(205, word, SemanticLocalRoleV1::Argument(2)),
        local(206, slice, SemanticLocalRoleV1::Temporary),
        local(207, word, SemanticLocalRoleV1::Temporary),
        local(208, slice, SemanticLocalRoleV1::Temporary),
    ];
    if let Some(opaque) = opaque {
        arguments.push(SemanticAbiArgumentV1::source(value_abi(&types, opaque)));
        ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
        locals.push(local(209, opaque, SemanticLocalRoleV1::Argument(3)));
    }
    let predicate = locals.len() as u32;
    locals.push(local(210, boolean, SemanticLocalRoleV1::Temporary));
    let abi = SemanticFunctionAbiV1::from_rustc(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        arguments.len() as u32,
        arguments,
        original.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let switch = |left, right| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(predicate, boolean)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, left),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, right),
        )
        .unwrap(),
    };
    let copy = |local| SemanticOperandV1::Copy(place(local, slice));
    let set = |local, operand| assign(place(local, slice), SemanticRvalueKindV1::Use(operand));
    let length = || {
        assign(
            place(5, word),
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: copy(4),
            },
        )
    };
    let call = |operand, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![operand],
                Some(SemanticCallDestinationV1::new(
                    place(destination, slice),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let mut blocks = match flow {
        DescriptorFlowV29::Root => vec![block(
            211,
            vec![set(4, copy(1)), length()],
            SemanticTerminatorKindV1::Return,
        )],
        DescriptorFlowV29::Branch | DescriptorFlowV29::Mixed => {
            let other = if mixed {
                SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(7),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), slice)
                                .unwrap(),
                        ],
                        slice,
                    )
                    .unwrap(),
                )
            } else {
                copy(2)
            };
            vec![
                block(211, vec![], switch(1, 2)),
                block(212, vec![set(4, copy(1))], goto(3)),
                block(213, vec![set(4, other)], goto(3)),
                block(214, vec![length()], call(copy(4), 6, 4)),
                block(215, vec![], SemanticTerminatorKindV1::Return),
            ]
        }
        DescriptorFlowV29::Loop => vec![
            block(211, vec![set(4, copy(1))], goto(1)),
            block(212, vec![length()], switch(2, 3)),
            block(213, vec![set(4, copy(2))], goto(1)),
            block(214, vec![], call(copy(4), 6, 4)),
            block(215, vec![], SemanticTerminatorKindV1::Return),
        ],
        DescriptorFlowV29::RepeatedHelper => vec![
            block(211, vec![], call(copy(1), 4, 1)),
            block(212, vec![length()], call(copy(2), 6, 2)),
            block(
                213,
                vec![set(4, copy(6)), length()],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    };
    let mut entry_statements = blocks[0].statements().to_vec();
    entry_statements.insert(
        0,
        assign(
            place(predicate, boolean),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: SemanticOperandV1::Copy(place(3, word)),
                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    word,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                )),
            },
        ),
    );
    blocks[0] = block(211, entry_statements, blocks[0].terminator().kind().clone());
    let root = function(200, SemanticFunctionRoleV1::KernelRoot, abi, locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = vec![root];
    let mut callables = vec![SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(0),
    )];
    if !matches!(flow, DescriptorFlowV29::Root) {
        let value = original.abi().arguments()[0].value().clone();
        let SemanticAbiPassModeV1::Pair { second, .. } = value.mode() else {
            panic!("source slice ABI")
        };
        let pointee = types[slice.index() as usize]
            .abi_properties()
            .first_pointee()
            .unwrap();
        let returned_slice = SemanticAbiValueV1::new(
            slice,
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    (pointee.reliable_alignment_bytes() > 1)
                        .then_some(pointee.reliable_alignment_bytes()),
                )
                .unwrap(),
                second: *second,
            },
        );
        let helper_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([231; 32]),
            original.abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(value)],
            returned_slice,
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
        .unwrap();
        functions.push(function(
            230,
            SemanticFunctionRoleV1::InternalHelper,
            helper_abi,
            vec![
                local(232, slice, SemanticLocalRoleV1::Return),
                local(233, slice, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(
                234,
                vec![set(0, copy(1))],
                SemanticTerminatorKindV1::Return,
            )],
        ));
        callables.push(SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(1),
        ));
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
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

fn with_descriptor_flow_prepared_v29<R>(
    flow: DescriptorFlowV29,
    profiled: bool,
    preexisting: bool,
    budget: &mut ArgumentBudgetV1<'_>,
    visit: impl FnOnce(ProductionPreparedSourceV18, &mut ArgumentBudgetV1<'_>) -> R,
) -> R {
    with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        preexisting,
        budget,
        || descriptor_flow_owner_v29(flow),
        |owner, launch, input, _, budget| {
            let prepared = if profiled {
                let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
                let roots = fixture.roots();
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            } else {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            }
            .unwrap_or_else(|error| {
                panic!("{flow:?}, profiled={profiled}, preexisting={preexisting}: {error:?}")
            });
            visit(prepared, budget)
        },
    )
}

#[test]
fn authentic_descriptor_profile_reaches_root_helpers_cfg_loops_and_whole_returns() {
    for flow in [
        DescriptorFlowV29::Root,
        DescriptorFlowV29::Branch,
        DescriptorFlowV29::Loop,
        DescriptorFlowV29::RepeatedHelper,
        DescriptorFlowV29::Mixed,
    ] {
        for profiled in [false, true] {
            for preexisting in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                budget.reserve_storage(MODULE_FLOOR).unwrap();
                let mut visited = false;
                with_descriptor_flow_prepared_v29(
                    flow,
                    profiled,
                    preexisting,
                    &mut budget,
                    |prepared, budget| {
                        prepared.with_checked_source_v18(budget, |source, budget| {
                        visited = true;
                        let semantic = source.source_semantic(budget)?;
                        let original = &semantic.functions()[0];
                        for &ty in &original.abi().source_input_types()[..2] {
                            let SemanticTypeShapeV1::Pointer(pointer) = semantic.types()[ty.index() as usize].shape() else { panic!("source slice") };
                            assert_eq!(pointer.address_space(), 0);
                        }
                        let canonical = source.canonical(budget)?;
                        let (_, root) = source.root(0, budget)?;
                        let root = &canonical.module().functions[root];
                        let expected = if profiled { AddressSpace::Global } else { AddressSpace::Generic };
                        for ty in &root.signature.parameters[..2] {
                            assert!(matches!(ty, Type::Slice(slice) if slice.address_space == expected));
                        }
                        if matches!(flow, DescriptorFlowV29::Mixed) {
                            assert_eq!(original.abi().source_argument_ownership()[3],
                                SemanticSourceArgumentOwnershipV1::ByValue);
                            assert_eq!(root.signature.parameters.len(), 4);
                            assert!(matches!(&root.signature.parameters[3], Type::Slice(slice)
                                if slice.address_space == AddressSpace::Generic
                                    && slice.access == AccessMode::ReadOnly));
                        }
                        let helper_count = match flow { DescriptorFlowV29::Root => 0, DescriptorFlowV29::RepeatedHelper => 2, _ => 1 };
                        assert_eq!(source.instance_count(0, budget)?, helper_count + 1);
                        // Helpers have been expanded into the root. Authenticate
                        // each original invocation and its real return edge,
                        // rather than expecting obsolete helper definitions.
                        for instance in 1..=helper_count {
                            assert!(source.instance_active(0, instance, budget)?);
                            let (function, incoming) = source.instance(0, instance, budget)?;
                            assert_eq!(function, SemanticFunctionIdV1::from_index(1));
                            assert_eq!(incoming.unwrap().0, 0);
                            let mut mapped = std::collections::BTreeSet::new();
                            for ordinal in 0..source.span_count(0, budget)? {
                                for segment in 0..2 {
                                    if let Some((owner, block, _, _)) = source.span_segment(0, ordinal, segment, budget)?
                                        && owner == instance { mapped.insert(block); }
                                }
                            }
                            assert_eq!(mapped.len(), 1, "one original helper block per invocation");
                            let body = root.body.as_ref().unwrap();
                            let block = body.blocks.iter().find(|row| mapped.contains(&row.id)).unwrap();
                            assert!(block.parameters.is_empty());
                            let mut entries = body.blocks.iter().filter(|row|
                                matches!(&row.terminator, Some(Terminator::Branch { target, arguments })
                                    if *target == block.id && arguments.is_empty()));
                            let entry = entries.next().expect("unique scoped invocation preheader");
                            assert!(entries.next().is_none());
                            let expected = if matches!(flow, DescriptorFlowV29::Mixed) { AddressSpace::Generic } else { expected };
                            assert!(matches!(&entry.parameters[..], [ValueDef { ty: Type::Slice(slice), .. }]
                                if slice.address_space == expected && slice.access == AccessMode::ReadOnly));
                            let Some(Terminator::Branch { target, arguments }) = &block.terminator else { panic!("original helper return continuation") };
                            assert_eq!(arguments, &vec![entry.parameters[0].id]);
                            let continuation = body.blocks.iter().find(|row| row.id == *target).unwrap();
                            assert_eq!(continuation.parameters.len(), 1);
                            assert_eq!(continuation.parameters[0].ty, entry.parameters[0].ty);
                        }
                        let mut widenings = 0;
                        let mut metadata = 0;
                        for function in &canonical.module().functions {
                            for block in &function.body.as_ref().unwrap().blocks {
                                for operation in &block.operations {
                                    match &operation.kind {
                                        OperationKind::Cast { kind: CastKind::SliceToGeneric, to, .. } => {
                                            widenings += 1;
                                            assert!(matches!(to, Type::Slice(slice) if slice.address_space == AddressSpace::Generic));
                                        }
                                        OperationKind::SliceLength { .. } => metadata += 1,
                                        _ => {}
                                    }
                                }
                            }
                        }
                        assert!(metadata > 0);
                        assert_eq!(widenings > 0, profiled && matches!(flow, DescriptorFlowV29::Mixed));
                        Ok(())
                    }).unwrap_or_else(|error| panic!("{flow:?}, profiled={profiled}, preexisting={preexisting}: {error:?}"));
                    },
                );
                assert!(visited, "actual source replay must execute for {flow:?}");
                assert_eq!(budget.storage(), MODULE_FLOOR);
                budget.release_storage(MODULE_FLOOR).unwrap();
            }
        }
    }
}

#[test]
fn original_descriptor_profile_rejects_same_type_argument_and_component_substitution() {
    for fault in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_pending_api_owner_v18(
            ModuleFixture::Ordinary,
            false,
            &mut budget,
            || descriptor_flow_owner_v29(DescriptorFlowV29::Branch),
            |owner, launch, input, _, budget| {
                let mut fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
                let arguments = fixture.arguments_mut(0);
                match fault {
                    0 => arguments.swap(0, 1),
                    1 => {
                        arguments.pop();
                    }
                    4 => {
                        arguments[0].kind =
                            ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 0 }
                    }
                    2 | 3 => {}
                    _ => unreachable!(),
                }
                let mut roots = fixture.roots();
                let foreign = [119; 32];
                match fault {
                    2 => roots[0].kernel_binding = &foreign,
                    3 => roots[0].explicit_argument_bytes -= 1,
                    0 | 1 | 4 => {}
                    _ => unreachable!(),
                }
                let result = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(), budget,
                );
                assert!(
                    matches!(
                        result,
                        Err(EntranceError::Source(
                            ProductionPendingScopedSourceErrorV29::Source(
                                ProductionSemanticKirErrorV1::Unsupported { .. }
                            )
                        ))
                    ),
                    "foreign profile fault {fault} must fail before emission"
                );
            },
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_replay_rejects_a_structurally_valid_same_type_foreign_descriptor_cast_input() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut pending = with_pending_api_owner_v18(
        ModuleFixture::Ordinary,
        false,
        &mut budget,
        || descriptor_flow_owner_v29(DescriptorFlowV29::Mixed),
        |owner, launch, input, _, budget| {
            let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = fixture.roots();
            ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap()
        },
    );
    pending.replay_with_budget(&mut budget).unwrap();
    let original_identity = *pending.pending_identity();
    let (mut candidate, candidate_storage) = pending
        .inner
        .pending
        .graph
        .copy_module_for_transformation_v18(&mut budget)
        .unwrap();
    budget
        .reserve_storage(candidate_storage.retained_storage())
        .unwrap();
    let root = candidate
        .functions
        .iter_mut()
        .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
        .unwrap();
    assert_eq!(root.signature.parameters[0], root.signature.parameters[1]);
    let body = root.body.as_mut().unwrap();
    let known = [body.parameters[0], body.parameters[1]];
    let mut changed = false;
    for block in &mut body.blocks {
        for operation in &mut block.operations {
            if let OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                value,
                ..
            } = &mut operation.kind
            {
                let replacement = if *value == known[0] {
                    known[1]
                } else {
                    known[0]
                };
                assert_ne!(*value, replacement);
                *value = replacement;
                changed = true;
                break;
            }
        }
        if changed {
            break;
        }
    }
    assert!(changed, "the mixed source must contain a genuine widening");
    let (changed, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::
        from_module_ref_with_verification_budget_v18(&candidate,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(candidate);
    budget
        .release_storage(candidate_storage.retained_storage())
        .unwrap();
    let old_credit = pending.inner.pending.graph_storage.retained_storage();
    let new_credit = receipt.retained_storage();
    pending.inner.retained_storage = pending.inner.retained_storage - old_credit + new_credit;
    pending.inner.pending.retained_storage =
        pending.inner.pending.retained_storage - old_credit + new_credit;
    let old = std::mem::replace(&mut pending.inner.pending.graph, changed);
    drop(old);
    budget.release_storage(old_credit).unwrap();
    pending.inner.pending.graph_storage = receipt;
    assert_ne!(pending.pending_identity(), &original_identity);
    let floor = budget.storage();
    assert!(
        pending.replay_with_budget(&mut budget).is_err(),
        "matching descriptor types cannot substitute the original current source and archive"
    );
    assert_eq!(budget.storage(), floor);
    let credit = pending.adopted_storage();
    drop(pending);
    budget.release_storage(credit).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
