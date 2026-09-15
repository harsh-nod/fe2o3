mod execution_source_occurrence_tests {
    use super::*;
    use fe2o3_kernel_ir::ExecutionCapabilitySourceOccurrenceV1;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn repeated_helper_source(calls: u32) -> ProductionSemanticMirOwnerV1 {
        assert!((1..=2).contains(&calls));
        let (base, _) = numerical_policy_tests::fixture_with_policy_move(false);
        let semantic = base.semantic();
        let original = &semantic.functions()[0];
        let source = SemanticSourceProvenanceV1::unavailable();
        let unit = SemanticTypeIdV1::from_index(0);
        let context = SemanticTypeIdV1::from_index(1);
        let reference = SemanticTypeIdV1::from_index(2);
        let capability = SemanticTypeIdV1::from_index(3);
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let call = |callee, arguments, local, ty, target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        place(local, ty),
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
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let mut blocks = vec![block(100, vec![], call(2, vec![], 1, context, 1))];
        for invocation in 0..calls {
            blocks.push(block(
                101 + invocation as u8,
                if invocation == 0 {
                    original.blocks()[1].statements().to_vec()
                } else {
                    vec![]
                },
                call(
                    1,
                    vec![SemanticOperandV1::Copy(place(2, reference))],
                    0,
                    unit,
                    invocation + 2,
                ),
            ));
        }
        blocks.push(block(104, vec![], SemanticTerminatorKindV1::Return));
        let root = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            original.abi().clone(),
            original.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = &semantic.callables()[2]
        else {
            unreachable!()
        };
        let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1::from_sha256([110; 32]),
            binding.abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![reference],
            unit,
            binding.abi().arguments().to_vec(),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
        .unwrap();
        let helper = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([111; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([112; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([113; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([114; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([115; 32]),
            source,
            abi,
            [
                (unit, SemanticLocalRoleV1::Return),
                (reference, SemanticLocalRoleV1::Argument(0)),
                (capability, SemanticLocalRoleV1::Temporary),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([116 + index as u8; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
            SemanticBlockIdV1::from_index(0),
            vec![
                block(
                    120,
                    vec![],
                    call(
                        3,
                        vec![SemanticOperandV1::Copy(place(1, reference))],
                        2,
                        capability,
                        1,
                    ),
                ),
                block(121, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            vec![],
            vec![],
            vec![],
            vec![root, helper],
            vec![
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
                semantic.callables()[1].clone(),
                semantic.callables()[2].clone(),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    fn lower(source: ProductionSemanticMirOwnerV1) -> ProductionSemanticKirOwnerV1 {
        ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            source,
            ProductionSemanticKirLimitsV1::default(),
            vec![ProductionKernelContextLoweringInputV1::new(
                SemanticFunctionIdV1::from_index(0),
                [4; 32],
                [1; 32],
                [2; 32],
                [3; 32],
                [7; 32],
            )],
        )
        .unwrap()
    }

    fn sources(owner: &ProductionSemanticKirOwnerV1) -> Vec<ExecutionCapabilitySourceV1> {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|op| match &op.kind {
                OperationKind::ExecutionCapability(contract) => Some(contract.source),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn repeated_helper_terminal_uses_actual_original_site_and_distinct_instances() {
        let owner = lower(repeated_helper_source(2));
        owner.verify_equivalence().unwrap();
        let sources = sources(&owner);
        assert_eq!(sources.len(), 2);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.semantic_ssa.execution_view_for_root(root).unwrap();
        for source in &sources {
            let occurrence = source.occurrence.unwrap();
            assert_eq!(source.function, [111; 32]);
            assert_eq!(source.operation, [51; 32]);
            assert_eq!(source.block, 0);
            assert_eq!(
                occurrence.root_source_identity(),
                *owner.semantic_ssa.source_semantic().functions()[0]
                    .identity()
                    .as_bytes()
            );
            assert_eq!(
                occurrence.expansion_identity(),
                *owner.semantic_ssa.execution_expansion().identity()
            );
            assert_eq!(occurrence.expanded_root_identity(), *view.identity());
            let origin = &view.block_origins()[occurrence.expanded_block() as usize];
            assert_eq!(occurrence.caller_instance(), origin.instance().index());
            assert_eq!(origin.function(), SemanticFunctionIdV1::from_index(1));
            assert_eq!(origin.block().index(), source.block);
            assert_eq!(
                origin.terminator(),
                fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source
            );
        }
        assert_ne!(
            sources[0].occurrence.unwrap().caller_instance(),
            sources[1].occurrence.unwrap().caller_instance()
        );
        assert_ne!(
            sources[0].occurrence.unwrap().expanded_block(),
            sources[1].occurrence.unwrap().expanded_block()
        );
        assert_ne!(
            sources[0].occurrence.unwrap().expansion_identity(),
            sources[0].occurrence.unwrap().expanded_root_identity()
        );
    }

    #[test]
    fn unexpanded_terminal_retains_original_source_without_occurrence() {
        let (source, _) = numerical_policy_tests::fixture_with_policy_move(false);
        let owner = lower(source);
        owner.verify_equivalence().unwrap();
        let sources = sources(&owner);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].occurrence, None);
        assert_eq!(
            sources[0].function,
            *owner.semantic_ssa.source_semantic().functions()[0]
                .identity()
                .as_bytes()
        );
        assert_eq!(sources[0].block, 1);
    }

    #[test]
    fn carrier_rejects_root_whole_expansion_view_site_callee_and_call_substitution() {
        let root = SemanticFunctionIdV1::from_index(0);
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            repeated_helper_source(2),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let function = execution_function_for_root_v1(&ssa, root).unwrap();
        let view = ssa.execution_view_for_root(root).unwrap();
        let whole = execution_expansion_identity_v1(&ssa);
        let expanded = Some(*view.identity());
        let carrier = CheckedExecutionSourceCarrierV1::new(&ssa, root, function).unwrap();
        let calls = function
            .blocks()
            .iter()
            .enumerate()
            .filter_map(|(index, b)| match b.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) if call.callee().index() == 3 => {
                    Some((SemanticBlockIdV1::from_index(index as u32), call))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let (block, call) = calls[0];
        let callee = SemanticFunctionIdentityV1::from_sha256([51; 32]);
        assert!(
            carrier
                .source_for_call(root, function, whole, expanded, block, call, callee)
                .is_ok()
        );
        for (index, origin) in view.block_origins().iter().enumerate() {
            if origin.terminator() != fe2o3_mir_model::SemanticExpandedTerminatorOriginV1::Source {
                assert!(
                    carrier
                        .source_for_call(
                            root,
                            function,
                            whole,
                            expanded,
                            SemanticBlockIdV1::from_index(index as u32),
                            call,
                            callee
                        )
                        .is_err()
                );
            }
        }
        for (root, whole, expanded, site, actual_call, callee) in [
            (
                SemanticFunctionIdV1::from_index(1),
                whole,
                expanded,
                block,
                call,
                callee,
            ),
            (root, Some([201; 32]), expanded, block, call, callee),
            (root, None, expanded, block, call, callee),
            (root, expanded, whole, block, call, callee),
            (root, whole, Some([202; 32]), block, call, callee),
            (root, whole, None, block, call, callee),
            (
                root,
                whole,
                expanded,
                SemanticBlockIdV1::from_index(u32::MAX),
                call,
                callee,
            ),
            (root, whole, expanded, calls[1].0, call, callee),
            (root, whole, expanded, block, calls[1].1, callee),
            (
                root,
                whole,
                expanded,
                block,
                call,
                SemanticFunctionIdentityV1::from_sha256([203; 32]),
            ),
        ] {
            assert!(
                carrier
                    .source_for_call(root, function, whole, expanded, site, actual_call, callee)
                    .is_err()
            );
        }
        let foreign = ProductionSemanticSsaOwnerV1::try_new(
            repeated_helper_source(1),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        assert!(CheckedExecutionSourceCarrierV1::new(&foreign, root, function).is_err());
        assert!(
            CheckedExecutionSourceCarrierV1::new(&ssa, root, &ssa.source_semantic().functions()[0])
                .is_err()
        );
    }

    #[test]
    fn retained_producer_replay_rejects_each_occurrence_and_original_source_substitution() {
        let mut owner = lower(repeated_helper_source(2));
        let original_module = owner.module.clone();
        let original = sources(&owner)[0];
        let occurrence = original.occurrence.unwrap();
        let make = |root, expansion, view, instance, block| {
            Some(
                ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    root, expansion, view, instance, block,
                )
                .unwrap(),
            )
        };
        for changed in [
            None,
            make(
                [201; 32],
                occurrence.expansion_identity(),
                occurrence.expanded_root_identity(),
                occurrence.caller_instance(),
                occurrence.expanded_block(),
            ),
            make(
                occurrence.root_source_identity(),
                [202; 32],
                occurrence.expanded_root_identity(),
                occurrence.caller_instance(),
                occurrence.expanded_block(),
            ),
            make(
                occurrence.root_source_identity(),
                occurrence.expansion_identity(),
                [203; 32],
                occurrence.caller_instance(),
                occurrence.expanded_block(),
            ),
            make(
                occurrence.root_source_identity(),
                occurrence.expansion_identity(),
                occurrence.expanded_root_identity(),
                u32::MAX,
                occurrence.expanded_block(),
            ),
            make(
                occurrence.root_source_identity(),
                occurrence.expansion_identity(),
                occurrence.expanded_root_identity(),
                occurrence.caller_instance(),
                u32::MAX,
            ),
        ] {
            owner.module = original_module.clone();
            first_contract(&mut owner.module).source.occurrence = changed;
            assert!(owner.verify_equivalence().is_err());
        }
        for changed in [
            ExecutionCapabilitySourceV1 {
                function: [204; 32],
                ..original
            },
            ExecutionCapabilitySourceV1 {
                operation: [205; 32],
                ..original
            },
            ExecutionCapabilitySourceV1 {
                block: 1,
                ..original
            },
        ] {
            owner.module = original_module.clone();
            first_contract(&mut owner.module).source = changed;
            assert!(owner.verify_equivalence().is_err());
        }
        owner.module = original_module;
        owner.verify_equivalence().unwrap();
    }

    fn first_contract(module: &mut Module) -> &mut ExecutionCapabilityOpV1 {
        module
            .functions
            .iter_mut()
            .filter_map(|f| f.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|op| match &mut op.kind {
                OperationKind::ExecutionCapability(contract) => Some(contract),
                _ => None,
            })
            .unwrap()
    }
}
