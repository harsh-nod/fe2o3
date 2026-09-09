mod frame_initialization_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn rebuild_function(
        original: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        let rebuilt = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            locals,
            original.entry(),
            blocks,
        )
        .unwrap();
        match original.kernel_entry() {
            Some(entry) => rebuilt.with_kernel_entry(entry.clone()),
            None => rebuilt,
        }
    }

    fn scope_helper_source(unreachable_calls: bool) -> ProductionSemanticMirOwnerV1 {
        let baseline = helper_closure_semantic_owner_with_calls(2);
        let semantic = baseline.semantic();
        let source = SemanticSourceProvenanceV1::unavailable();
        let unit = SemanticTypeIdV1::from_index(0);
        let scope = SemanticTypeIdV1::from_index(1);
        let reference = SemanticTypeIdV1::from_index(2);
        let pipeline = SemanticTypeIdV1::from_index(3);
        let pipeline_reference = SemanticTypeIdV1::from_index(4);
        let index = SemanticTypeIdV1::from_index(5);
        let zst = |tag| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            )
        };
        let reference_type = |tag, pointee| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                            0,
                            1,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            )
        };
        let types = vec![
            semantic.types()[0].clone(),
            zst(210),
            reference_type(211, scope),
            zst(212),
            reference_type(213, pipeline),
            plain_bit_scalar_type(
                214,
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
        ];
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                source,
            )
        };
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(
            index,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
        ));
        let helper = rebuild_function(
            &semantic.functions()[1],
            vec![
                local(216, unit, SemanticLocalRoleV1::Return),
                local(217, scope, SemanticLocalRoleV1::Temporary),
                local(218, reference, SemanticLocalRoleV1::Temporary),
                local(219, pipeline, SemanticLocalRoleV1::Temporary),
                local(220, pipeline_reference, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([220; 32]),
                    source,
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(2, reference),
                            SemanticRvalueV1::new(
                                reference,
                                SemanticRvalueKindV1::Borrow {
                                    kind: SemanticBorrowKindV1::Mutable,
                                    place: place(1, scope),
                                },
                            ),
                        )),
                    )],
                    SemanticTerminatorV1::new(
                        source,
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(2),
                                vec![SemanticOperandV1::Copy(place(2, reference))],
                                Some(SemanticCallDestinationV1::new(
                                    place(3, pipeline),
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::CallReturn,
                                        SemanticBlockIdV1::from_index(1),
                                    ),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([221; 32]),
                    source,
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(4, pipeline_reference),
                            SemanticRvalueV1::new(
                                pipeline_reference,
                                SemanticRvalueKindV1::Borrow {
                                    kind: SemanticBorrowKindV1::Mutable,
                                    place: place(3, pipeline),
                                },
                            ),
                        )),
                    )],
                    SemanticTerminatorV1::new(
                        source,
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable(
                                SemanticCallableIdV1::from_index(3),
                                vec![
                                    SemanticOperandV1::Copy(place(4, pipeline_reference)),
                                    zero.clone(),
                                    zero.clone(),
                                    zero,
                                ],
                                Some(SemanticCallDestinationV1::new(
                                    place(0, unit),
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::CallReturn,
                                        SemanticBlockIdV1::from_index(2),
                                    ),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([222; 32]),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ],
        );
        let reference_argument = |ty| {
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
            )
        };
        let scalar = SemanticAbiValueV1::new(
            index,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        );
        let intrinsic =
            |tag, arguments, output, operation| SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                    source,
                    SemanticFunctionAbiV1::new(
                        SemanticAbiIdentityV1::from_sha256([tag; 32]),
                        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                        SemanticCanonAbiV1::Rust,
                        false,
                        false,
                        arguments,
                        output,
                    )
                    .unwrap(),
                ),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
            };
        let mut root =
            semantic.functions()[0]
                .clone()
                .with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(b"helper_root".to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256([210; 32]),
                    SemanticKernelSourceContractV1::new(
                        Some(
                            SemanticKernelLaunchBoundsV1::new(
                                Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                                Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                                None,
                            )
                            .unwrap(),
                        ),
                        None,
                        None,
                    )
                    .unwrap(),
                ));
        if unreachable_calls {
            let mut blocks = root.blocks().to_vec();
            blocks[0] = SemanticBasicBlockV1::new(
                blocks[0].identity(),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap();
            root = rebuild_function(&root, root.locals().to_vec(), blocks);
        }
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types,
            vec![],
            vec![],
            vec![],
            vec![root, helper],
            vec![
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
                intrinsic(
                    223,
                    vec![reference_argument(reference)],
                    SemanticAbiValueV1::new(pipeline, SemanticAbiPassModeV1::Ignore),
                    SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                        scope,
                        pipeline,
                        buffers: 2,
                        elements: 64,
                        prefetch_distance: 1,
                    },
                ),
                intrinsic(
                    224,
                    vec![
                        reference_argument(pipeline_reference),
                        scalar.clone(),
                        scalar.clone(),
                        scalar,
                    ],
                    SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
                    SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                        pipeline,
                        element: index,
                    },
                ),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    fn scope_helper_ssa(unreachable_calls: bool) -> ProductionSemanticSsaOwnerV1 {
        ProductionSemanticSsaOwnerV1::try_new(
            scope_helper_source(unreachable_calls),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn lowering(owner: &ProductionSemanticSsaOwnerV1) -> SemanticFunctionLoweringV1<'_> {
        let root = SemanticFunctionIdV1::from_index(0);
        SemanticFunctionLoweringV1::new_interprocedural(
            owner.source_semantic().types(),
            owner.source_semantic().callables(),
            owner.execution_view_for_root(root).unwrap().body(),
            owner.execution_plan_for_root(root).unwrap(),
            root,
            root,
            BTreeMap::new(),
            BTreeMap::new(),
            vec![],
            SemanticParameterBindingsV1 {
                declarations: &[],
                values: &[],
                types: &[],
                local_bindings: None,
            },
            None,
            Some([1, 1, 1]),
            BTreeSet::new(),
            1,
            false,
            None,
            true,
            owner.source_semantic().target().object_size_bound_bytes(),
            65_536,
        )
        .unwrap()
    }

    #[test]
    fn repeated_ambient_helper_calls_lower_and_replay_with_distinct_origins() {
        let source = scope_helper_source(false);
        let source_bytes = source.semantic().canonical_encoding().to_vec();
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        let plan = lowered
            .semantic_ssa
            .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let records = plan.frame_initializations();
        assert_eq!(records.len(), 2);
        assert_ne!(records[0].local(), records[1].local());
        assert_ne!(
            records[0].origin().instance(),
            records[1].origin().instance()
        );
        assert!(plan.implicit_entry_variables().is_empty());
        assert_eq!(
            lowered.semantic().semantic().canonical_encoding(),
            source_bytes
        );
        assert_eq!(
            lowered.module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::WorkgroupMemory(_)))
                .count(),
            2
        );
        for record in records {
            assert!(
                lowered
                    .correspondence
                    .statement_operation_spans
                    .iter()
                    .any(|span| span.semantic_block == record.block()
                        && span.statement_ordinal == record.statement()
                        && span.operation_count == 0)
            );
        }
        lowered.verify_equivalence().unwrap();
    }

    #[test]
    fn scope_is_initialized_only_after_its_marker_and_uses_the_planned_ssa_definition() {
        let owner = scope_helper_ssa(false);
        let mut lower = lowering(&owner);
        let records = lower
            .pending_frame_initializations
            .values()
            .copied()
            .collect::<Vec<_>>();
        let entry = lower.function.entry();
        lower
            .begin_block(entry, &mut BasicBlock::new(BlockId(entry.index())))
            .unwrap();
        assert!(lower.control_flow_ssa.implicit_entry_locals.is_empty());
        assert!(
            records
                .iter()
                .all(|row| lower.locals[row.local().index() as usize].is_none())
        );
        let mut operations = vec![];
        for row in records {
            let local = row.local().index() as usize;
            let site = (row.block().index(), row.local().index());
            let definition = lower.pending_semantic_ssa_definitions[&site][0];
            lower.retained_local_initialized.insert(row.local().index());
            lower
                .lower_statement(
                    row.block(),
                    Some(row.statement()),
                    &SemanticStatementKindV1::StorageLive(row.local()),
                    &mut operations,
                )
                .unwrap();
            assert!(matches!(
                lower.locals[local],
                Some(SemanticValueBindingV1::WorkgroupLdsScope)
            ));
            assert!(matches!(
                lower.semantic_ssa_bindings.get(&definition),
                Some(SemanticValueBindingV1::WorkgroupLdsScope)
            ));
            assert!(
                !lower
                    .retained_local_initialized
                    .contains(&row.local().index())
            );
            assert!(lower.pending_semantic_ssa_definitions[&site].is_empty());
            lower
                .lower_statement(
                    row.block(),
                    None,
                    &SemanticStatementKindV1::StorageDead(row.local()),
                    &mut operations,
                )
                .unwrap();
            assert!(lower.locals[local].is_none());
            lower
                .lower_statement(
                    row.block(),
                    Some(row.statement()),
                    &SemanticStatementKindV1::StorageLive(row.local()),
                    &mut operations,
                )
                .unwrap();
            assert!(lower.locals[local].is_none());
        }
        assert!(operations.is_empty());
        lower.require_frame_initializations_consumed().unwrap();
    }

    #[test]
    fn stale_plan_rejects_missing_moved_dead_or_wrong_local_markers() {
        let owner = scope_helper_ssa(false);
        let lower = lowering(&owner);
        let plan = owner
            .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let row = plan.frame_initializations()[0];
        let other = plan.frame_initializations()[1];
        for replacement in [
            None,
            Some(SemanticStatementKindV1::Nop),
            Some(SemanticStatementKindV1::StorageDead(row.local())),
            Some(SemanticStatementKindV1::StorageLive(other.local())),
        ] {
            let mut blocks = lower.function.blocks().to_vec();
            let block = &blocks[row.block().index() as usize];
            let mut statements = block.statements().to_vec();
            if let Some(kind) = replacement {
                statements[row.statement() as usize] =
                    SemanticStatementV1::new(block.source(), kind);
            } else {
                statements.insert(
                    row.statement() as usize,
                    SemanticStatementV1::new(block.source(), SemanticStatementKindV1::Nop),
                );
            }
            blocks[row.block().index() as usize] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
            let stale = rebuild_function(lower.function, lower.function.locals().to_vec(), blocks);
            assert!(matches!(
                plan_frame_initializations_v1(
                    lower.types,
                    lower.callables,
                    &stale,
                    plan,
                    &lower.control_flow_ssa
                ),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        }
    }

    #[test]
    fn consumer_rejects_wrong_coordinates_and_does_not_consume_missing_markers() {
        let owner = scope_helper_ssa(false);
        for move_block in [false, true] {
            let mut lower = lowering(&owner);
            let (site, row) = lower.pending_frame_initializations.pop_first().unwrap();
            let wrong_site = if move_block {
                (site.0 + 1, site.1)
            } else {
                (site.0, site.1 + 1)
            };
            lower.pending_frame_initializations.insert(wrong_site, row);
            assert!(matches!(
                lower.lower_statement(
                    SemanticBlockIdV1::from_index(wrong_site.0),
                    Some(wrong_site.1),
                    &SemanticStatementKindV1::StorageLive(row.local()),
                    &mut vec![]
                ),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert!(lower.locals[row.local().index() as usize].is_none());
        }
        let mut lower = lowering(&owner);
        let row = *lower.pending_frame_initializations.values().next().unwrap();
        lower
            .lower_statement(
                row.block(),
                Some(row.statement()),
                &SemanticStatementKindV1::Nop,
                &mut vec![],
            )
            .unwrap();
        assert!(lower.locals[row.local().index() as usize].is_none());
        assert!(matches!(
            lower.require_frame_initializations_consumed(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(
            require_semantic_ssa_definitions_consumed_v1(
                lower.semantic_function.index(),
                &lower.pending_semantic_ssa_definitions
            )
            .is_err()
        );
    }

    #[test]
    fn frame_rows_require_nominal_scope_authentication_and_promoted_transport() {
        let owner = scope_helper_ssa(false);
        let lower = lowering(&owner);
        let plan = owner
            .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        let row = plan.frame_initializations()[0];
        let declaration = &lower.function.locals()[row.local().index() as usize];
        for (ty, role) in [
            (
                SemanticTypeIdV1::from_index(3),
                SemanticLocalRoleV1::Temporary,
            ),
            (declaration.ty(), SemanticLocalRoleV1::Return),
        ] {
            let mut locals = lower.function.locals().to_vec();
            locals[row.local().index() as usize] =
                SemanticLocalDeclV1::new(declaration.identity(), ty, role, declaration.source());
            let stale = rebuild_function(lower.function, locals, lower.function.blocks().to_vec());
            assert!(matches!(
                plan_frame_initializations_v1(
                    lower.types,
                    lower.callables,
                    &stale,
                    plan,
                    &lower.control_flow_ssa
                ),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        }
        for remove_binding in [false, true] {
            let mut transport = lower.control_flow_ssa.clone();
            if remove_binding {
                transport.compiler_issued_bindings.remove(&declaration.ty());
            } else {
                transport.ssa_value_locals.remove(&row.local().index());
            }
            assert!(matches!(
                plan_frame_initializations_v1(
                    lower.types,
                    lower.callables,
                    lower.function,
                    plan,
                    &transport
                ),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        }
    }

    #[test]
    fn unreachable_frame_records_are_preserved_without_requiring_execution() {
        let owner = scope_helper_ssa(true);
        let root = SemanticFunctionIdV1::from_index(0);
        let plan = owner.execution_plan_for_root(root).unwrap();
        assert!(!plan.frame_initializations().is_empty());
        let lower = lowering(&owner);
        assert!(lower.pending_frame_initializations.is_empty());
        lower.require_frame_initializations_consumed().unwrap();
        assert!(lower.locals.iter().all(Option::is_none));
    }
}
