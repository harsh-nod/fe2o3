mod guarded_value_tests {
    use super::*;
    use fe2o3_kernel_ir::{GlobalCapabilityBindV1, GlobalCapabilityIndexV1, IntegerSwitchCase};
    use fe2o3_mir_model::semantic_mir_v1::*;

    const LOAD: FunctionOperationLocation = FunctionOperationLocation::new(BlockId(0), 10);
    const STORE: FunctionOperationLocation = FunctionOperationLocation::new(BlockId(1), 0);
    const SITE: SemanticAccessSiteV1 = SemanticAccessSiteV1 {
        block: 0,
        statement: None,
        ordinal: 0,
    };

    struct Fixture {
        module: Module,
        authority: KirScalarCorrelationAuthorityV1,
        sites: BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
    }

    fn fixture() -> Fixture {
        let context = KernelContextTypeV1::new("guarded_scalar", [1; 32], [2; 32], [3; 32]);
        let capability = GlobalCapabilityTypeV1::new(
            Type::Scalar(ScalarType::F32),
            context.clone(),
            GlobalCapabilityRoleV1::ExclusiveReadWrite,
        );
        let mut entry = BasicBlock::new(BlockId(0));
        let value = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
        let mut access = MemoryAccess::new(AddressSpace::Global, 4);
        access.volatile = true;
        entry.operations = vec![
            value(
                3,
                Type::KernelContext(context),
                OperationKind::KernelContextIssue(KernelContextIssueV1::new(
                    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
                )),
            ),
            value(
                4,
                Type::GlobalCapability(capability.clone()),
                OperationKind::GlobalCapabilityBind(GlobalCapabilityBindV1 {
                    context: ValueId(3),
                    physical: ValueId(0),
                }),
            ),
            value(
                5,
                Type::INDEX,
                OperationKind::GlobalCapabilityIndex(GlobalCapabilityIndexV1 {
                    capability: ValueId(4),
                    index: ValueId(1),
                    index_space: None,
                }),
            ),
            value(
                6,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(4) },
            ),
            value(
                7,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(5),
                    rhs: ValueId(6),
                },
            ),
            value(8, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            value(
                9,
                Type::INDEX,
                OperationKind::Select {
                    condition: ValueId(7),
                    true_value: ValueId(5),
                    false_value: ValueId(8),
                },
            ),
            value(
                10,
                capability.physical_pointer_type(),
                OperationKind::SliceData { slice: ValueId(4) },
            ),
            value(
                11,
                capability.physical_pointer_type(),
                OperationKind::GetElementPointer {
                    base: ValueId(10),
                    offset: ValueId(9),
                },
            ),
            value(
                12,
                Type::Scalar(ScalarType::F32),
                OperationKind::Constant(Constant::F32Bits(0)),
            ),
            value(
                13,
                Type::Scalar(ScalarType::F32),
                OperationKind::GuardedLoad {
                    pointer: ValueId(11),
                    predicate: ValueId(7),
                    fallback: ValueId(12),
                    access,
                },
            ),
            value(
                14,
                Type::Scalar(ScalarType::F32),
                OperationKind::Constant(Constant::F32Bits(0x3f80_0000)),
            ),
            value(
                15,
                Type::Scalar(ScalarType::F32),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(13),
                    rhs: ValueId(14),
                },
            ),
        ];
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(7),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut store = BasicBlock::new(BlockId(1));
        store.operations.push(Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(11),
                predicate: ValueId(7),
                value: ValueId(15),
                access,
            },
        ));
        store.terminator = Some(Terminator::Return { values: vec![] });
        let mut exit = BasicBlock::new(BlockId(2));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("guarded_scalar");
        module.functions.push(Function::kernel_entry(
            "guarded_scalar",
            Signature::new(
                vec![capability.physical_slice_type(), Type::INDEX, Type::BOOL],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![entry, store, exit],
        ));
        module.kernels.push(Kernel::new(
            "guarded_scalar",
            "guarded_scalar",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        verify_module(&module).unwrap();
        Fixture {
            module,
            authority: KirScalarCorrelationAuthorityV1 {
                capabilities: BTreeMap::from([(ValueId(4), capability.clone())]),
                loads: BTreeMap::from([(SITE, capability)]),
            },
            sites: BTreeMap::from([((LOAD, 0), SITE)]),
        }
    }

    fn body(fixture: &mut Fixture) -> &mut FunctionBody {
        fixture.module.functions[0].body.as_mut().unwrap()
    }

    fn normalize(
        fixture: &Fixture,
        consumer: FunctionOperationLocation,
        value: ValueId,
    ) -> Option<NormalizedScalarExpressionV1> {
        let function = &fixture.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 65_536 };
        let kir = build_kir_correlation_index(function.body.as_ref()?, 256, &mut budget)?;
        normalize_kir_expression_v1(
            function,
            &kir,
            &fixture.sites,
            &KirScalarUseContextV1 {
                consumer,
                authority: &fixture.authority,
            },
            value,
            0,
            &mut BTreeSet::new(),
            &mut budget,
        )
    }

    fn expected_load() -> NormalizedScalarExpressionV1 {
        NormalizedScalarExpressionV1::Load {
            site: SITE,
            scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        }
    }

    fn unconditional_entry(fixture: &mut Fixture) {
        body(fixture).blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });
    }

    fn store_predicate(fixture: &mut Fixture, predicate: ValueId) {
        let OperationKind::GuardedStore {
            predicate: current, ..
        } = &mut body(fixture).blocks[1].operations[0].kind
        else {
            unreachable!()
        };
        *current = predicate;
    }

    #[test]
    fn checked_some_load_normalizes_inside_an_arithmetic_write_expression() {
        let fixture = fixture();
        assert_eq!(
            normalize(&fixture, STORE, ValueId(15)),
            Some(NormalizedScalarExpressionV1::Binary {
                operation: ProductionSemanticBinaryOpV2::Add,
                scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(expected_load()),
                rhs: Box::new(NormalizedScalarExpressionV1::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                    bits: 0x3f80_0000,
                }),
            })
        );
    }

    #[test]
    fn exact_option_discriminant_switch_proves_some_but_reversed_or_bypass_edges_do_not() {
        let mut fixture = fixture();
        store_predicate(&mut fixture, ValueId(2));
        body(&mut fixture).blocks[0].operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(16), Type::INDEX),
                OperationKind::Constant(Constant::Index(1)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(17), Type::INDEX),
                OperationKind::Select {
                    condition: ValueId(7),
                    true_value: ValueId(16),
                    false_value: ValueId(8),
                },
            ),
        ]);
        body(&mut fixture).blocks[0].terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(17),
            cases: vec![IntegerSwitchCase {
                value: Constant::Index(0),
                target: BlockId(2),
                arguments: vec![],
            }],
            default_target: BlockId(1),
            default_arguments: vec![],
        });
        assert_eq!(
            normalize(&fixture, STORE, ValueId(13)),
            Some(expected_load())
        );
        let Terminator::IntegerSwitch { cases, .. } =
            body(&mut fixture).blocks[0].terminator.as_mut().unwrap()
        else {
            unreachable!()
        };
        cases[0].target = BlockId(1);
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
        unconditional_entry(&mut fixture);
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
    }

    #[test]
    fn store_conjunction_can_prove_a_load_predicate_but_disjunction_cannot() {
        let mut fixture = fixture();
        unconditional_entry(&mut fixture);
        body(&mut fixture).blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(16), Type::BOOL),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(7),
                    rhs: ValueId(2),
                },
            ));
        store_predicate(&mut fixture, ValueId(16));
        assert_eq!(
            normalize(&fixture, STORE, ValueId(13)),
            Some(expected_load())
        );
        let OperationKind::Binary { op, .. } =
            &mut body(&mut fixture).blocks[0].operations[13].kind
        else {
            unreachable!()
        };
        *op = BinaryOp::BitOr;
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
    }

    #[test]
    fn consumers_do_not_share_guard_facts_and_unguarded_fallback_is_not_a_load() {
        let mut fixture = fixture();
        unconditional_entry(&mut fixture);
        let mut second = body(&mut fixture).blocks[1].operations[0].clone();
        let OperationKind::GuardedStore { predicate, .. } = &mut second.kind else {
            unreachable!()
        };
        *predicate = ValueId(2);
        body(&mut fixture).blocks[1].operations.push(second);
        assert_eq!(
            normalize(&fixture, STORE, ValueId(13)),
            Some(expected_load())
        );
        assert!(
            normalize(
                &fixture,
                FunctionOperationLocation::new(BlockId(1), 1),
                ValueId(13)
            )
            .is_none()
        );
        let OperationKind::GuardedStore {
            pointer,
            value,
            access,
            ..
        } = second_store(&fixture).kind
        else {
            unreachable!()
        };
        body(&mut fixture).blocks[1].operations[1].kind = OperationKind::Store {
            pointer,
            value,
            access,
        };
        assert!(
            normalize(
                &fixture,
                FunctionOperationLocation::new(BlockId(1), 1),
                ValueId(13)
            )
            .is_none()
        );
    }

    fn second_store(fixture: &Fixture) -> &Operation {
        &fixture.module.functions[0].body.as_ref().unwrap().blocks[1].operations[1]
    }

    #[test]
    fn ssa_merge_requires_the_same_loaded_origin_on_every_incoming_edge() {
        let mut fixture = fixture();
        let mut store = body(&mut fixture).blocks[1].clone();
        store.id = BlockId(3);
        store.parameters = vec![ValueDef::new(ValueId(16), Type::Scalar(ScalarType::F32))];
        let OperationKind::GuardedStore { value, .. } = &mut store.operations[0].kind else {
            unreachable!()
        };
        *value = ValueId(16);
        body(&mut fixture).blocks[1].operations.clear();
        for block in &mut body(&mut fixture).blocks[1..] {
            block.terminator = Some(Terminator::Branch {
                target: BlockId(3),
                arguments: vec![ValueId(13)],
            });
        }
        body(&mut fixture).blocks.push(store);
        verify_module(&fixture.module).unwrap();
        let consumer = FunctionOperationLocation::new(BlockId(3), 0);
        assert_eq!(
            normalize(&fixture, consumer, ValueId(16)),
            Some(expected_load())
        );
        let Some(Terminator::Branch { arguments, .. }) =
            &mut body(&mut fixture).blocks[2].terminator
        else {
            unreachable!()
        };
        arguments[0] = ValueId(12);
        verify_module(&fixture.module).unwrap();
        assert!(normalize(&fixture, consumer, ValueId(16)).is_none());
    }

    #[test]
    fn guarded_recipe_mutations_reject_even_when_the_original_guard_dominates() {
        let mutations: [fn(&mut Fixture); 9] = [
            |fixture| {
                body(fixture).blocks[0].operations[9].kind =
                    OperationKind::Constant(Constant::F32Bits(1))
            },
            |fixture| {
                let OperationKind::GuardedLoad { predicate, .. } =
                    &mut body(fixture).blocks[0].operations[10].kind
                else {
                    unreachable!()
                };
                *predicate = ValueId(2);
            },
            |fixture| {
                let OperationKind::GuardedLoad { access, .. } =
                    &mut body(fixture).blocks[0].operations[10].kind
                else {
                    unreachable!()
                };
                access.volatile = false;
            },
            |fixture| {
                let OperationKind::Select { false_value, .. } =
                    &mut body(fixture).blocks[0].operations[6].kind
                else {
                    unreachable!()
                };
                *false_value = ValueId(1);
            },
            |fixture| {
                let OperationKind::GlobalCapabilityIndex(index) =
                    &mut body(fixture).blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                index.capability = ValueId(0);
            },
            |fixture| {
                let OperationKind::SliceLength { slice } =
                    &mut body(fixture).blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                *slice = ValueId(0);
            },
            |fixture| {
                fixture.authority.capabilities.clear();
            },
            |fixture| {
                fixture.authority.loads.clear();
            },
            |fixture| {
                fixture
                    .sites
                    .insert((LOAD, 0), SemanticAccessSiteV1 { block: 1, ..SITE });
            },
        ];
        for mutation in mutations {
            let mut fixture = fixture();
            mutation(&mut fixture);
            assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
        }
    }

    #[test]
    fn typed_index_trace_preserves_the_exact_contract_and_requires_root_authority() {
        let mut fixture = fixture();
        let function = &fixture.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        assert_eq!(
            checked_global_index_operand_v1(&kir, &fixture.authority, ValueId(5), &mut budget),
            Some(ValueId(1))
        );
        assert_eq!(
            guard_index_origin_v1(function, &kir, &fixture.authority, ValueId(5), &mut budget),
            Some((ValueId(1), None))
        );
        let mapping =
            GlobalDisjointIndexContractV1::new([42; 32], GlobalDisjointIndexSpaceV1::Index1d);
        let OperationKind::GlobalCapabilityIndex(index) =
            &mut body(&mut fixture).blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        index.index_space = Some(mapping);
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
        let OperationKind::GlobalCapabilityIndex(index) =
            &mut body(&mut fixture).blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        index.index_space = None;
        let other_root = GlobalCapabilityTypeV1::new(
            Type::Scalar(ScalarType::F32),
            KernelContextTypeV1::new("another_root", [11; 32], [12; 32], [13; 32]),
            GlobalCapabilityRoleV1::ReadOnly,
        );
        fixture
            .authority
            .capabilities
            .insert(ValueId(4), other_root);
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
    }

    #[test]
    fn equivalent_raw_index_and_physical_length_guard_match_only_the_authenticated_projection() {
        let mut fixture = fixture();
        unconditional_entry(&mut fixture);
        body(&mut fixture).blocks[0].operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(16), Type::INDEX),
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(17), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(1),
                    rhs: ValueId(16),
                },
            ),
        ]);
        store_predicate(&mut fixture, ValueId(17));
        assert_eq!(
            normalize(&fixture, STORE, ValueId(13)),
            Some(expected_load())
        );
        let OperationKind::Compare { lhs, .. } =
            &mut body(&mut fixture).blocks[0].operations[14].kind
        else {
            unreachable!()
        };
        *lhs = ValueId(8);
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
    }

    #[test]
    fn cycles_missing_definitions_and_budget_exhaustion_fail_closed() {
        let mut fixture = fixture();
        body(&mut fixture).blocks[1].terminator = Some(Terminator::Branch {
            target: BlockId(0),
            arguments: vec![],
        });
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
        body(&mut fixture).blocks[1].terminator = Some(Terminator::Return { values: vec![] });
        let function = &fixture.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        let context = KirScalarUseContextV1 {
            consumer: STORE,
            authority: &fixture.authority,
        };
        budget.remaining = 0;
        assert!(
            checked_guarded_scalar_load_v1(function, &kir, &context, LOAD, SITE, &mut budget)
                .is_none()
        );
        let mut missing = self::fixture();
        body(&mut missing).blocks[0].operations.remove(9);
        assert!(normalize(&missing, STORE, ValueId(13)).is_none());
    }

    #[test]
    fn load_must_execute_before_the_write_on_every_path() {
        let mut fixture = fixture();
        let original = body(&mut fixture).blocks.clone();
        let mut entry = BasicBlock::new(BlockId(0));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut loaded = original[0].clone();
        loaded.id = BlockId(1);
        loaded.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
        let mut bypass = BasicBlock::new(BlockId(2));
        bypass.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
        let mut store = original[1].clone();
        store.id = BlockId(3);
        body(&mut fixture).blocks = vec![entry, loaded, bypass, store];
        fixture.sites =
            BTreeMap::from([((FunctionOperationLocation::new(BlockId(1), 10), 0), SITE)]);
        assert!(
            normalize(
                &fixture,
                FunctionOperationLocation::new(BlockId(3), 0),
                ValueId(13)
            )
            .is_none()
        );
        let mut wrong_order = self::fixture();
        let store = body(&mut wrong_order).blocks[1].operations.remove(0);
        body(&mut wrong_order).blocks[0]
            .operations
            .insert(10, store);
        let function = &wrong_order.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        assert!(
            prove_guarded_value_at_consumer_v1(
                function,
                &kir,
                &KirScalarUseContextV1 {
                    consumer: FunctionOperationLocation::new(BlockId(0), 10),
                    authority: &wrong_order.authority
                },
                FunctionOperationLocation::new(BlockId(0), 11),
                ValueId(7),
                &mut budget
            )
            .is_none()
        );
    }

    #[test]
    fn source_global_type_is_bound_to_original_root_and_binding_identity() {
        let source = scalar_transmute_semantic_owner();
        let source = source.semantic();
        let entry = source.functions()[0].kernel_entry().unwrap();
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            entry.kernel_binding_identity(),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([4; 32]),
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([2; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        let mut execution = SemanticKirExecutionInputV1 {
            source,
            function: &source.functions()[0],
            root: SemanticFunctionIdV1::from_index(0),
            source_body: SemanticFunctionIdV1::from_index(0),
            expansion_identity: None,
        };
        assert!(
            source_global_scalar_type_v1(
                execution,
                SemanticTypeIdV1::from_index(1),
                SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance
            )
            .is_some()
        );
        let wrong_binding = SemanticKernelCapabilityProvenanceV1::new(
            provenance.root(),
            SemanticKernelBindingIdentityV1::from_sha256([255; 32]),
            provenance.frontend_unit(),
            provenance.kernel_marker(),
            provenance.target_brand(),
            provenance.launch_brand(),
            provenance.issuance(),
        )
        .unwrap();
        assert!(
            source_global_scalar_type_v1(
                execution,
                SemanticTypeIdV1::from_index(1),
                SemanticCapabilityMemoryContractV1::global_read_only(),
                wrong_binding,
            )
            .is_none()
        );
        execution.root = SemanticFunctionIdV1::from_index(1);
        assert!(
            source_global_scalar_type_v1(
                execution,
                SemanticTypeIdV1::from_index(1),
                SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance
            )
            .is_none()
        );
    }

    #[test]
    fn distinct_allocations_do_not_share_bounds_even_on_an_equal_length_path() {
        let mut fixture = fixture();
        let capability = fixture.authority.capabilities[&ValueId(4)].clone();
        fixture.module.functions[0]
            .signature
            .parameters
            .push(capability.physical_slice_type());
        body(&mut fixture).parameters.push(ValueId(16));
        body(&mut fixture).blocks[0].operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(17), Type::GlobalCapability(capability.clone())),
                OperationKind::GlobalCapabilityBind(GlobalCapabilityBindV1 {
                    context: ValueId(3),
                    physical: ValueId(16),
                }),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(18), Type::INDEX),
                OperationKind::SliceLength { slice: ValueId(17) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(19), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(5),
                    rhs: ValueId(18),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(20), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(6),
                    rhs: ValueId(18),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(21), Type::BOOL),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(20),
                    rhs: ValueId(19),
                },
            ),
        ]);
        fixture
            .authority
            .capabilities
            .insert(ValueId(17), capability);
        let Some(Terminator::ConditionalBranch { condition, .. }) =
            &mut body(&mut fixture).blocks[0].terminator
        else {
            unreachable!()
        };
        *condition = ValueId(21);
        let OperationKind::GuardedStore {
            pointer,
            value,
            access,
            ..
        } = body(&mut fixture).blocks[1].operations[0].kind
        else {
            unreachable!()
        };
        body(&mut fixture).blocks[1].operations[0].kind = OperationKind::Store {
            pointer,
            value,
            access,
        };
        verify_module(&fixture.module).unwrap();
        assert!(normalize(&fixture, STORE, ValueId(13)).is_none());
        let function = &fixture.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        assert_eq!(
            guard_extent_origin_v1(&kir, &fixture.authority, ValueId(6), &mut budget),
            Some(ValueId(0))
        );
        assert_eq!(
            guard_extent_origin_v1(&kir, &fixture.authority, ValueId(18), &mut budget),
            Some(ValueId(16))
        );
        // Keep the same equality branch, changing only the bound to this load's allocation.
        let OperationKind::Binary { rhs, .. } =
            &mut body(&mut fixture).blocks[0].operations[17].kind
        else {
            unreachable!()
        };
        *rhs = ValueId(7);
        assert_eq!(
            normalize(&fixture, STORE, ValueId(13)),
            Some(expected_load())
        );
    }

    #[test]
    fn guard_index_casts_preserve_only_unsigned_non_narrowing_values() {
        for (intermediate, first, second, accepted) in [
            (ScalarType::U64, CastKind::Bitcast, CastKind::Bitcast, true),
            (
                ScalarType::U32,
                CastKind::Truncate,
                CastKind::ZeroExtend,
                false,
            ),
            (
                ScalarType::I32,
                CastKind::Truncate,
                CastKind::SignExtend,
                false,
            ),
            (ScalarType::I64, CastKind::Bitcast, CastKind::Bitcast, false),
        ] {
            let mut fixture = fixture();
            unconditional_entry(&mut fixture);
            body(&mut fixture).blocks[0].operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(16), Type::Scalar(intermediate)),
                    OperationKind::Cast {
                        kind: first,
                        value: ValueId(1),
                        to: Type::Scalar(intermediate),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(17), Type::INDEX),
                    OperationKind::Cast {
                        kind: second,
                        value: ValueId(16),
                        to: Type::INDEX,
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(18), Type::BOOL),
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(17),
                        rhs: ValueId(6),
                    },
                ),
            ]);
            store_predicate(&mut fixture, ValueId(18));
            assert_eq!(
                normalize(&fixture, STORE, ValueId(13)).is_some(),
                accepted,
                "{intermediate:?} {first:?} {second:?}"
            );
        }
    }

    #[test]
    fn nonzero_budget_exhaustion_during_guard_replay_fails_closed() {
        let fixture = fixture();
        let function = &fixture.module.functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 65_536 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        let context = KirScalarUseContextV1 {
            consumer: STORE,
            authority: &fixture.authority,
        };
        let initial = budget.remaining;
        assert_eq!(
            checked_guarded_scalar_load_v1(function, &kir, &context, LOAD, SITE, &mut budget),
            Some(())
        );
        let cost = initial - budget.remaining;
        assert!(cost > 4);
        for remaining in [1, cost / 2, cost - 1] {
            let mut limited = UnsupportedIndexCorrelationBudgetV1 { remaining };
            assert!(
                checked_guarded_scalar_load_v1(function, &kir, &context, LOAD, SITE, &mut limited)
                    .is_none(),
                "budget {remaining} of {cost}"
            );
            assert_eq!(limited.remaining, 0);
        }
        let mut exact = UnsupportedIndexCorrelationBudgetV1 { remaining: cost };
        assert_eq!(
            checked_guarded_scalar_load_v1(function, &kir, &context, LOAD, SITE, &mut exact),
            Some(())
        );
        assert_eq!(exact.remaining, 0);
    }

    fn admitted_global_load_owner() -> ProductionSemanticKirOwnerV1 {
        let [
            unit,
            element,
            context,
            context_ref,
            slice,
            physical,
            view,
            view_ref,
            index,
            tag,
            option,
        ] = std::array::from_fn(|i| SemanticTypeIdV1::from_index(i as u32));
        let baseline = noop_semantic_owner(&["guarded_authority"]);
        let original = &baseline.semantic().functions()[0];
        let source = original.source();
        let pointer_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        );
        let length_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let pair = SemanticBackendReprV1::scalar_pair(pointer_scalar, length_scalar);
        let pointer_properties = SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: false },
                        0,
                        1,
                    )
                    .unwrap(),
                ),
                None,
            );
        let declaration = |identity, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([identity; 32]),
                SemanticLayoutIdentityV1::from_sha256([identity; 32]),
                layout,
                shape,
            )
        };
        let reference = |identity, pointee, wide| {
            declaration(
                identity,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(if wide { 16 } else { 8 }),
                    8,
                    if wide {
                        pair
                    } else {
                        SemanticBackendReprV1::scalar(pointer_scalar)
                    },
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        if wide {
                            SemanticPointerMetadataV1::SliceLength
                        } else {
                            SemanticPointerMetadataV1::None
                        },
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(pointer_properties)
        };
        let physical_type = reference(48, slice, true);
        let physical_layout = physical_type.layout();
        let view_type = declaration(
            50,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                physical_layout.rustc_size_bytes(),
                physical_layout.alignment_bytes(),
                SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                *physical_layout.backend_repr(),
                physical_layout.largest_niche(),
                physical_layout.is_uninhabited(),
                physical_layout.max_repr_alignment_bytes(),
                physical_layout.unadjusted_abi_alignment_bytes(),
                physical_layout.randomization_seed(),
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                ),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![physical]).unwrap()),
        )
        .with_rustc_abi_properties(pointer_properties);
        let variant = |variant, offsets: Vec<u64>| {
            SemanticEnumVariantLayoutV1::from_rustc(
                variant,
                8,
                4,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        };
        let types = vec![
            unit_type(),
            plain_bit_scalar_type(
                40,
                SemanticBackendPrimitiveV1::float(32, 4),
                SemanticScalarTypeV1::Float { bits: 32 },
            ),
            declaration(
                42,
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ),
            reference(44, context, false),
            declaration(
                46,
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
                SemanticTypeShapeV1::Slice { element },
            ),
            physical_type,
            view_type,
            reference(52, view, false),
            plain_bit_scalar_type(
                54,
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
            plain_bit_scalar_type(
                56,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            declaration(
                58,
                SemanticTypeLayoutV1::enum_layout(
                    8,
                    4,
                    SemanticEnumLayoutV1::new(
                        vec![variant(0, vec![]), variant(1, vec![4])],
                        SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                            0,
                            0,
                            SemanticBackendScalarV1::initialized(
                                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                                SemanticScalarValidityRangeV1::new(0, 1),
                            ),
                        )),
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::enum_type(
                    tag,
                    vec![
                        SemanticEnumVariantV1::new(
                            0,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            1,
                            SemanticAggregateTypeV1::new(vec![element]).unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            ),
        ];
        let attributes = |non_null| {
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, non_null, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap()
        };
        let abi_value = |ty| {
            SemanticAbiValueV1::new(
                ty,
                if ty == unit || ty == context {
                    SemanticAbiPassModeV1::Ignore
                } else if ty == physical || ty == view {
                    SemanticAbiPassModeV1::Pair {
                        first: attributes(true),
                        second: attributes(false),
                    }
                } else if ty == option {
                    SemanticAbiPassModeV1::Cast {
                        pad_i32: false,
                        cast: SemanticAbiCastV1::new(
                            [None; 8],
                            None,
                            SemanticAbiUniformV1::new(
                                SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8)
                                    .unwrap(),
                                8,
                            )
                            .unwrap(),
                            SemanticAbiValueAttributesV1::plain(),
                        ),
                    }
                } else {
                    SemanticAbiPassModeV1::Direct(attributes(ty == context_ref || ty == view_ref))
                },
            )
        };
        let abi = |identity, inputs: Vec<SemanticTypeIdV1>, output, ownership, root| {
            SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([identity; 32]),
                SemanticLayoutIdentityV1::from_sha256([250; 32]),
                if root {
                    SemanticCanonAbiV1::GpuKernel
                } else {
                    SemanticCanonAbiV1::Rust
                },
                if root {
                    SemanticExternAbiV1::GpuKernel
                } else {
                    SemanticExternAbiV1::Rust
                },
                false,
                false,
                inputs.len() as u32,
                inputs
                    .into_iter()
                    .map(|ty| SemanticAbiArgumentV1::source(abi_value(ty)))
                    .collect(),
                abi_value(output),
            )
            .unwrap()
            .with_source_argument_ownership(ownership)
            .unwrap()
        };
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            original.kernel_entry().unwrap().kernel_binding_identity(),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([4; 32]),
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([2; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        let intrinsic = |identity, inputs, output, ownership, operation| {
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([identity; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([identity; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([identity; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([identity; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([identity; 32]),
                    source,
                    abi(identity, inputs, output, ownership, false),
                ),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(
                    [identity; 32],
                ),
            }
        };
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            intrinsic(
                120,
                vec![],
                context,
                vec![],
                SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
            ),
            intrinsic(
                130,
                vec![context_ref, physical],
                view,
                vec![SemanticSourceArgumentOwnershipV1::SharedBorrow; 2],
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    context,
                    physical,
                    view,
                    element,
                    provenance,
                    contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                    source_identity: SemanticFunctionIdentityV1::from_sha256([130; 32]),
                },
            ),
            intrinsic(
                140,
                vec![view_ref, index],
                option,
                vec![
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
                    view,
                    option,
                    element,
                    provenance,
                    contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                    source_identity: SemanticFunctionIdentityV1::from_sha256([140; 32]),
                },
            ),
        ];
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let borrow = |destination, result, local, ty| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(destination, result),
                    SemanticRvalueV1::new(
                        result,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(local, ty),
                        },
                    ),
                )),
            )
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
        let block = |identity, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([identity; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            abi(
                100,
                vec![physical],
                unit,
                vec![SemanticSourceArgumentOwnershipV1::SharedBorrow],
                true,
            ),
            [unit, physical, context, context_ref, view, view_ref, option]
                .into_iter()
                .enumerate()
                .map(|(local, ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([100 + local as u8; 32]),
                        ty,
                        match local {
                            0 => SemanticLocalRoleV1::Return,
                            1 => SemanticLocalRoleV1::Argument(0),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                        source,
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            vec![
                block(110, vec![], call(1, vec![], 2, context, 1)),
                block(
                    111,
                    vec![borrow(3, context_ref, 2, context)],
                    call(
                        2,
                        vec![
                            SemanticOperandV1::Copy(place(3, context_ref)),
                            SemanticOperandV1::Copy(place(1, physical)),
                        ],
                        4,
                        view,
                        2,
                    ),
                ),
                block(
                    112,
                    vec![borrow(5, view_ref, 4, view)],
                    call(
                        3,
                        vec![
                            SemanticOperandV1::Copy(place(5, view_ref)),
                            SemanticOperandV1::Constant(SemanticConstantV1::new(
                                index,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(0, 8).unwrap(),
                                ),
                            )),
                        ],
                        6,
                        option,
                        3,
                    ),
                ),
                block(113, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let source = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types,
            vec![],
            vec![],
            vec![],
            vec![function],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_exact_v16(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
                .unwrap(),
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

    fn replay_authority(
        owner: &ProductionSemanticKirOwnerV1,
        module: &Module,
        correspondence: &SemanticKirCorrespondenceV1,
        remaining: usize,
    ) -> Option<KirScalarCorrelationAuthorityV1> {
        let execution = SemanticKirExecutionInputV1::for_root(
            &owner.semantic_ssa,
            SemanticFunctionIdV1::from_index(0),
        )
        .ok()?;
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining };
        let kir =
            build_kir_correlation_index(module.functions[0].body.as_ref()?, 256, &mut budget)?;
        let sites = index_semantic_access_sites(
            correspondence,
            execution.root,
            execution.source_body,
            &kir,
            &mut budget,
        )?;
        checked_kir_scalar_authority_v1(Some(execution), correspondence, &kir, &sites, &mut budget)
    }

    #[test]
    fn admitted_source_and_lowered_spans_replay_scalar_authority() {
        let owner = admitted_global_load_owner();
        let authority =
            replay_authority(&owner, owner.module(), owner.correspondence(), 65_536).unwrap();
        assert_eq!(authority.capabilities.len(), 1);
        assert_eq!(authority.loads.len(), 1);
        let expected = authority.capabilities.values().next().unwrap();
        assert_eq!(expected.role(), GlobalCapabilityRoleV1::ReadOnly);
        assert_eq!(expected.element(), &Type::Scalar(ScalarType::F32));
        assert_eq!(expected.context().root().as_str(), "guarded_authority");
        assert_eq!(authority.loads.values().next(), Some(expected));
        let function = &owner.module().functions[0];
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let kir =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 256, &mut budget).unwrap();
        let (&index, projected) = kir
            .definitions
            .iter()
            .find(|(_, operation)| {
                matches!(operation.kind, OperationKind::GlobalCapabilityIndex(_))
            })
            .unwrap();
        let OperationKind::GlobalCapabilityIndex(projected) = projected.kind else {
            unreachable!()
        };
        assert_eq!(
            checked_global_index_operand_v1(&kir, &authority, index, &mut budget),
            Some(projected.index)
        );
    }

    #[test]
    fn source_authority_replay_rejects_mutated_issuance_types_and_spans() {
        let owner = admitted_global_load_owner();
        for axis in 0..8 {
            let mut module = owner.module().clone();
            let mut correspondence = owner.correspondence().clone();
            let body = module.functions[0].body.as_mut().unwrap();
            let (block, ordinal) = body
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .enumerate()
                        .map(move |(ordinal, operation)| (block.id, ordinal, operation))
                })
                .find_map(|(block, ordinal, operation)| {
                    matches!(operation.kind, OperationKind::GlobalCapabilityBind(_))
                        .then_some((block, ordinal))
                })
                .unwrap();
            match axis {
                0 => {
                    let issue = body
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.operations)
                        .find(|operation| {
                            matches!(operation.kind, OperationKind::KernelContextIssue(_))
                        })
                        .unwrap();
                    issue.kind = OperationKind::KernelContextIssue(KernelContextIssueV1::new(
                        KernelContextSourceIdentityV1::new([4; 32], [255; 32], [240; 32], [7; 32]),
                    ));
                }
                1 => {
                    let bind = &mut body.blocks[block.0 as usize].operations[ordinal];
                    let Type::GlobalCapability(capability) = &bind.results[0].ty else {
                        unreachable!()
                    };
                    bind.results[0].ty = Type::GlobalCapability(GlobalCapabilityTypeV1::new(
                        Type::Scalar(ScalarType::F32),
                        capability.context().clone(),
                        GlobalCapabilityRoleV1::ExclusiveReadWrite,
                    ));
                }
                2 => {
                    let bind = &mut body.blocks[block.0 as usize].operations[ordinal];
                    let OperationKind::GlobalCapabilityBind(bind) = &mut bind.kind else {
                        unreachable!()
                    };
                    bind.context = bind.physical;
                }
                _ => {
                    let span = correspondence
                        .terminator_operation_spans
                        .iter_mut()
                        .find(|span| {
                            operation_span_contains_v1(
                                span.kernel_ir_block(),
                                span.first_operation_ordinal(),
                                span.operation_count(),
                                FunctionOperationLocation::new(block, ordinal),
                            )
                        })
                        .unwrap();
                    match axis {
                        3 => span.semantic_block = SemanticBlockIdV1::from_index(0),
                        4 => span.correspondence_owner = SemanticFunctionIdV1::from_index(1),
                        5 => span.semantic_function = SemanticFunctionIdV1::from_index(1),
                        6 => span.operation_count = 0,
                        7 => {
                            let duplicate = *span;
                            let mut spans = correspondence.terminator_operation_spans.to_vec();
                            spans.push(duplicate);
                            correspondence.terminator_operation_spans = spans.into_boxed_slice();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            assert!(
                replay_authority(&owner, &module, &correspondence, 65_536).is_none(),
                "mutation {axis}"
            );
        }
    }

    #[test]
    fn source_load_authority_requires_the_exact_terminator_access_site() {
        let owner = admitted_global_load_owner();
        let execution = SemanticKirExecutionInputV1::for_root(
            &owner.semantic_ssa,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 65_536 };
        let kir = build_kir_correlation_index(
            owner.module().functions[0].body.as_ref().unwrap(),
            256,
            &mut budget,
        )
        .unwrap();
        let sites = index_semantic_access_sites(
            owner.correspondence(),
            execution.root,
            execution.source_body,
            &kir,
            &mut budget,
        )
        .unwrap();
        let (&key, &site) = sites
            .iter()
            .find(|(key, _)| {
                matches!(
                    kir.operations[&key.0].kind,
                    OperationKind::GuardedLoad { .. }
                )
            })
            .unwrap();
        assert_eq!(site.block, 2);
        for changed in [
            SemanticAccessSiteV1 { block: 1, ..site },
            SemanticAccessSiteV1 {
                statement: Some(0),
                ..site
            },
            SemanticAccessSiteV1 { ordinal: 1, ..site },
        ] {
            let mut changed_sites = sites.clone();
            changed_sites.insert(key, changed);
            let authority = checked_kir_scalar_authority_v1(
                Some(execution),
                owner.correspondence(),
                &kir,
                &changed_sites,
                &mut budget,
            )
            .unwrap();
            assert_eq!(authority.capabilities.len(), 1);
            assert!(authority.loads.is_empty(), "{changed:?}");
        }
        let authority = checked_kir_scalar_authority_v1(
            Some(execution),
            owner.correspondence(),
            &kir,
            &sites,
            &mut budget,
        )
        .unwrap();
        assert!(authority.loads.contains_key(&site));
        let absent = checked_kir_scalar_authority_v1(
            None,
            owner.correspondence(),
            &kir,
            &sites,
            &mut budget,
        )
        .unwrap();
        assert!(absent.capabilities.is_empty() && absent.loads.is_empty());
    }

    #[test]
    fn source_authority_mid_replay_budget_exhaustion_does_not_release_partial_authority() {
        let owner = admitted_global_load_owner();
        let execution = SemanticKirExecutionInputV1::for_root(
            &owner.semantic_ssa,
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let mut setup = UnsupportedIndexCorrelationBudgetV1 { remaining: 65_536 };
        let kir = build_kir_correlation_index(
            owner.module().functions[0].body.as_ref().unwrap(),
            256,
            &mut setup,
        )
        .unwrap();
        let sites = index_semantic_access_sites(
            owner.correspondence(),
            execution.root,
            execution.source_body,
            &kir,
            &mut setup,
        )
        .unwrap();
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 4096 };
        let authority = checked_kir_scalar_authority_v1(
            Some(execution),
            owner.correspondence(),
            &kir,
            &sites,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            (authority.capabilities.len(), authority.loads.len()),
            (1, 1)
        );
        let cost = 4096 - budget.remaining;
        assert!(cost > 4);
        for remaining in [1, cost / 2, cost - 1] {
            let mut limited = UnsupportedIndexCorrelationBudgetV1 { remaining };
            assert!(
                checked_kir_scalar_authority_v1(
                    Some(execution),
                    owner.correspondence(),
                    &kir,
                    &sites,
                    &mut limited
                )
                .is_none(),
                "budget {remaining} of {cost}"
            );
            assert_eq!(limited.remaining, 0);
        }
        let mut exact = UnsupportedIndexCorrelationBudgetV1 { remaining: cost };
        assert!(
            checked_kir_scalar_authority_v1(
                Some(execution),
                owner.correspondence(),
                &kir,
                &sites,
                &mut exact
            )
            .is_some()
        );
        assert_eq!(exact.remaining, 0);
    }
}
