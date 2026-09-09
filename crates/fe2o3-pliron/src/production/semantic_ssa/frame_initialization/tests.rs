use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn repeated_scope_helper(cyclic: bool) -> AdmittedInertSemanticMirV1 {
    let baseline = super::super::tests::admitted_helper_semantic();
    let unit = SemanticTypeIdV1::from_index(0);
    let scope = SemanticTypeIdV1::from_index(1);
    let reference = SemanticTypeIdV1::from_index(2);
    let pipeline = SemanticTypeIdV1::from_index(3);
    let mut types = baseline.types().to_vec();
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
    types.push(zst(210));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([211; 32]),
            SemanticLayoutIdentityV1::from_sha256([211; 32]),
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
                    scope,
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
        ),
    );
    types.push(zst(212));
    let source = SemanticSourceProvenanceV1::unavailable();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            source,
        )
    };
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let call = |callee, arguments, destination, ty, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let rebuild = |original: &SemanticFunctionDeclV1, locals, blocks| {
        SemanticFunctionDeclV1::new(
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
        .unwrap()
    };
    let original = &baseline.functions()[0];
    let root = rebuild(
        original,
        original.locals().to_vec(),
        vec![
            block(213, vec![], call(1, vec![], 0, unit, 1)),
            block(214, vec![], call(1, vec![], 0, unit, 2)),
            block(
                215,
                vec![],
                if cyclic {
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(0),
                    ))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
        ],
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper = rebuild(
        &baseline.functions()[1],
        vec![
            local(216, unit, SemanticLocalRoleV1::Return),
            local(217, scope, SemanticLocalRoleV1::Temporary),
            local(218, reference, SemanticLocalRoleV1::Temporary),
            local(219, pipeline, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                220,
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
                call(
                    2,
                    vec![SemanticOperandV1::Copy(place(2, reference))],
                    3,
                    pipeline,
                    1,
                ),
            ),
            block(221, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([222; 32]),
        SemanticLayoutIdentityV1::from_sha256([222; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            reference,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )],
        SemanticAbiValueV1::new(pipeline, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let intrinsic = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([223; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([223; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([223; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([223; 32]),
            source,
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
            scope,
            pipeline,
            buffers: 2,
            elements: 64,
            prefetch_distance: 1,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([223; 32]),
    };
    InertSemanticMirRequestV1::new_with_callables(
        baseline.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            intrinsic,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

pub(crate) fn owner(cyclic: bool) -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            repeated_scope_helper(cyclic),
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn original_ambient_certificates_initialize_each_call_and_each_loop_iteration() {
    for cyclic in [false, true] {
        let owner = owner(cyclic);
        owner.verify_replay().unwrap();
        assert!(!owner.grants_proof_or_artifact_authority());
        let original = owner
            .plan_for_function(SemanticFunctionIdV1::from_index(1))
            .unwrap();
        assert_eq!(
            original.implicit_entry_variables(),
            [SsaVariableIdV1::new(1)]
        );
        assert!(original.frame_initializations().is_empty());
        let view = &owner.execution_expansion().roots()[0];
        let plan = owner.execution_plan_for_root(view.root()).unwrap();
        assert!(plan.implicit_entry_variables().is_empty());
        let entries = plan.frame_initializations();
        assert_eq!(entries.len(), 2);
        assert_ne!(
            entries[0].origin().instance(),
            entries[1].origin().instance()
        );
        assert_ne!(entries[0].local(), entries[1].local());
        let transparent =
            transparent_borrow_sites_v1(view.body(), owner.source_semantic().callables());
        let mut ranges = Vec::new();
        let (input, _, _) = adapter::semantic_function_ssa_input_with_frame_initializations_v1(
            view.body(),
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            &transparent,
            &plan.frame_initializations,
            |block, statement, events| {
                if entries.iter().any(|entry| {
                    entry.block().index() == block && Some(entry.statement()) == statement
                }) {
                    ranges.push((block, events));
                }
            },
        )
        .unwrap();
        for (entry, (block, events)) in entries.iter().zip(ranges) {
            let variable = SsaVariableIdV1::new(entry.local().index());
            assert_eq!(
                &input.blocks()[block as usize].events()[events],
                &[SsaEventV1::Kill(variable), SsaEventV1::Define(variable)]
            );
            assert!(
                input
                    .blocks()
                    .iter()
                    .any(|block| block.events().contains(&SsaEventV1::Kill(variable)))
            );
        }
        let (without, _, _) = semantic_function_ssa_input_v1(
            view.body(),
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            &transparent,
        );
        assert!(matches!(
            plan_ssa_with_limits_v1(&without, SsaPlannerLimitsV1::default()),
            Err(SsaPlannerErrorV1::UndefinedAtUse { .. })
        ));
    }
}

#[test]
fn stale_and_cross_instance_initialization_relations_are_rejected() {
    let owner = owner(false);
    let cyclic = self::owner(true);
    let view = &owner.execution_expansion().roots()[0];
    let relation = &owner
        .execution_plan_for_root(view.root())
        .unwrap()
        .frame_initializations;
    assert_eq!(
        relation.verify_view(&cyclic.execution_expansion().roots()[0]),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
    let mut crossed = relation.clone();
    crossed.entries[0].local = crossed.entries[1].local;
    crossed.entries[0].origin = crossed.entries[1].origin;
    assert_eq!(
        crossed.verify_view(view),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
    let mut digest = Sha256::new();
    relation.hash_into(&mut digest);
    let mut changed = Sha256::new();
    crossed.hash_into(&mut changed);
    assert_ne!(digest.finalize(), changed.finalize());
}

#[test]
fn deleted_frame_marker_cannot_receive_ambient_initialization() {
    let owner = owner(false);
    let view = &owner.execution_expansion().roots()[0];
    let relation = &owner
        .execution_plan_for_root(view.root())
        .unwrap()
        .frame_initializations;
    let entry = relation.entries()[0];
    let mut blocks = view.body().blocks().to_vec();
    let original = &blocks[entry.block().index() as usize];
    let mut statements = original.statements().to_vec();
    statements.remove(entry.statement() as usize);
    blocks[entry.block().index() as usize] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        original.terminator().clone(),
    )
    .unwrap();
    let original = view.body();
    let changed = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap();
    assert_eq!(
        relation.verify_markers(&changed),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
    let transparent = transparent_borrow_sites_v1(&changed, owner.source_semantic().callables());
    assert!(matches!(
        adapter::semantic_function_ssa_input_with_frame_initializations_v1(
            &changed,
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            &transparent,
            relation,
            |_, _, _| {},
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn frame_relation_storage_and_work_limits_are_inclusive() {
    let owner = owner(false);
    let view = &owner.execution_expansion().roots()[0];
    let expected = &owner
        .execution_plan_for_root(view.root())
        .unwrap()
        .frame_initializations;
    let defaults = SsaPlannerLimitsV1::default();
    let limits = |storage, work| {
        ProductionSemanticSsaLimitsV1::new(
            SsaPlannerLimitsV1::try_new(
                defaults.max_variables(),
                defaults.max_blocks(),
                defaults.max_edges(),
                defaults.max_events(),
                defaults.max_edge_definitions(),
                defaults.max_output_items(),
                storage,
                work,
            )
            .unwrap(),
        )
    };
    let resources = expected.resources();
    let derive = |limits| {
        FrameInitializationsV1::derive(owner.source_semantic(), view, owner.plans(), limits)
    };
    assert_eq!(
        &derive(limits(resources.storage_words, resources.work_units)).unwrap(),
        expected
    );
    for (storage, work, resource) in [
        (
            resources.storage_words - 1,
            resources.work_units,
            SsaPlannerResourceV1::StorageWords,
        ),
        (
            resources.storage_words,
            resources.work_units - 1,
            SsaPlannerResourceV1::WorkUnits,
        ),
    ] {
        assert!(matches!(derive(limits(storage, work)),
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit { resource: actual, .. }) if actual == resource));
    }
}
