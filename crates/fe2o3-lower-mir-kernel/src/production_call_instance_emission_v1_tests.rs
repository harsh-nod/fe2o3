    use super::*;

    fn fixture() -> (Function, Function, FunctionOperationLocation, BlockId) {
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let access = MemoryAccess::new(AddressSpace::Global, 4);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access,
            },
        ));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(3), scalar.clone())],
            OperationKind::Call {
                callee: FunctionId::new("leaf"),
                arguments: vec![ValueId(1), ValueId(2)],
            },
        ));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(3),
                access,
            },
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        let caller = Function::internal_helper(
            "caller",
            Signature::new(
                vec![pointer, scalar.clone(), Type::BOOL],
                vec![scalar.clone()],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        );
        let mut entry = BasicBlock::new(BlockId(17));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(101),
            then_target: BlockId(18),
            then_arguments: vec![],
            else_target: BlockId(19),
            else_arguments: vec![],
        });
        let mut left = BasicBlock::new(BlockId(18));
        left.terminator = Some(Terminator::Return {
            values: vec![ValueId(100)],
        });
        let mut right = BasicBlock::new(BlockId(19));
        right.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(102), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(100),
                rhs: ValueId(100),
            },
        ));
        right.terminator = Some(Terminator::Return {
            values: vec![ValueId(102)],
        });
        let callee = Function::internal_helper(
            "leaf",
            Signature::new(vec![scalar.clone(), Type::BOOL], vec![scalar]),
            vec![ValueId(100), ValueId(101)],
            vec![entry, left, right],
        );
        (
            caller,
            callee,
            FunctionOperationLocation::new(BlockId(0), 1),
            BlockId(20),
        )
    }

    fn run(
        caller: Function,
        callee: Function,
        site: FunctionOperationLocation,
        continuation: BlockId,
    ) -> Result<SplicedCallInstanceV1, CallInstanceEmissionErrorV1> {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        splice_production_call_instance_v1(
            caller,
            callee,
            site,
            BlockId(16),
            continuation,
            &mut budget,
        )
    }

    #[test]
    fn splice_preserves_effect_order_and_redirects_each_return() {
        let (caller, callee, site, continuation) = fixture();
        let result = run(caller, callee, site, continuation).unwrap();
        assert_eq!(result.split.returns, 2);
        assert_eq!(result.split.result_components, 1);
        assert!(result.callee_required_capabilities.is_empty());
        let blocks = &result.caller.body.as_ref().unwrap().blocks;
        assert_eq!(blocks[0].operations.len(), 1);
        assert!(matches!(
            blocks[0].operations[0].kind,
            OperationKind::Store {
                value: ValueId(1),
                ..
            }
        ));
        assert!(matches!(&blocks[0].terminator,
            Some(Terminator::Branch { target: BlockId(16), arguments })
                if arguments == &[ValueId(1), ValueId(2)]));
        assert_eq!(blocks[1].id, continuation);
        assert_eq!(
            blocks[1].parameters,
            [ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))]
        );
        assert!(matches!(
            blocks[1].operations[0].kind,
            OperationKind::Store {
                value: ValueId(3),
                ..
            }
        ));
        assert!(matches!(&blocks[1].terminator,
            Some(Terminator::Return { values }) if values == &[ValueId(3)]));
        assert_eq!(
            blocks[2]
                .parameters
                .iter()
                .map(|value| value.id)
                .collect::<Vec<_>>(),
            [ValueId(100), ValueId(101)]
        );
        assert!(blocks[3].parameters.is_empty());
        assert!(matches!(&blocks[2].terminator,
            Some(Terminator::Branch { target: BlockId(17), arguments }) if arguments.is_empty()));
        for block in &blocks[4..] {
            assert!(matches!(&block.terminator,
                Some(Terminator::Branch { target: BlockId(20), arguments }) if arguments.len() == 1));
        }
        let mut module = Module::new("spliced");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_accepts_zero_results_and_flattened_multiple_results() {
        for count in [0, 2] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            callee.signature.results = vec![scalar.clone(); count];
            let caller_body = caller.body.as_mut().unwrap();
            caller_body.blocks[0].operations[1].results = (0..count)
                .map(|slot| ValueDef::new(ValueId(3 + slot as u32), scalar.clone()))
                .collect();
            if count == 0 {
                caller.signature.results.clear();
                caller_body.blocks[0].operations.pop();
                caller_body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
            }
            for block in &mut callee.body.as_mut().unwrap().blocks {
                if let Some(Terminator::Return { values }) = &mut block.terminator {
                    values.resize(count, ValueId(100));
                }
            }
            let result = run(caller, callee, site, continuation).unwrap();
            assert_eq!(result.split.result_components, count);
            let mut module = Module::new("results");
            module.functions.push(result.caller);
            verify_module(&module).unwrap();
        }
    }

    #[test]
    fn splice_rejects_wrong_bindings_and_nonfresh_identities() {
        let (caller, mut callee, site, continuation) = fixture();
        callee.id = FunctionId::new("other");
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::WrongCallee)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.signature.parameters[1] = Type::Scalar(ScalarType::U64);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::SignatureMismatch)
        ));
        let (caller, callee, site, _) = fixture();
        assert!(matches!(
            run(caller, callee, site, BlockId(18)),
            Err(CallInstanceEmissionErrorV1::InvalidContinuation)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().parameters[0] = ValueId(1);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::IdentityRangeOverlap)
        ));
        let (caller, callee, _, continuation) = fixture();
        assert!(matches!(
            run(
                caller,
                callee,
                FunctionOperationLocation::new(BlockId(0), 0),
                continuation
            ),
            Err(CallInstanceEmissionErrorV1::MissingCall)
        ));
    }

    #[test]
    fn splice_accepts_disjoint_nonordered_block_and_value_ranges() {
        let (mut caller, callee, _, continuation) = fixture();
        caller
            .signature
            .parameters
            .push(Type::Scalar(ScalarType::U32));
        let body = caller.body.as_mut().unwrap();
        body.parameters.push(ValueId(1000));
        body.blocks[0].id = BlockId(30);
        let result = run(
            caller,
            callee,
            FunctionOperationLocation::new(BlockId(30), 1),
            continuation,
        )
        .unwrap();
        assert_eq!(result.split.entry, BlockId(16));
        assert_eq!(result.split.callee_entry, BlockId(17));
        let mut module = Module::new("disjoint");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_rejects_foreign_values_and_preserves_entry_backedges() {
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator = Some(Terminator::Return {
            values: vec![ValueId(1)],
        });
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::MissingDefinition)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator = Some(Terminator::Branch {
            target: BlockId(17),
            arguments: vec![],
        });
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("backedge");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_rejects_execution_results_even_without_a_return() {
        let (mut caller, callee, site, continuation) = fixture();
        caller.signature.results =
            vec![Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context)];
        caller.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::ExecutionTransport)
        ));
        let (caller, mut callee, site, continuation) = fixture();
        callee.body.as_mut().unwrap().blocks[1].terminator =
            Some(Terminator::Return { values: vec![] });
        assert!(matches!(
            run(caller, callee, site, continuation),
            Err(CallInstanceEmissionErrorV1::SignatureMismatch)
        ));
    }

    #[test]
    fn splice_rejects_callee_frame_allocations_including_repeated_calls() {
        for repeated in [false, true] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            let access = MemoryAccess::new(AddressSpace::Private, 4);
            let body = callee.body.as_mut().unwrap();
            body.blocks[0].operations = vec![
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(103),
                        Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    ),
                    OperationKind::Alloca {
                        element: scalar.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(103),
                        value: ValueId(100),
                        access,
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(104), scalar),
                    OperationKind::Load {
                        pointer: ValueId(103),
                        access,
                    },
                ),
            ];
            body.blocks[1].terminator = Some(Terminator::Return {
                values: vec![ValueId(104)],
            });
            if repeated {
                let body = caller.body.as_mut().unwrap();
                let mut returned = BasicBlock::new(BlockId(1));
                returned.terminator = body.blocks[0].terminator.take();
                body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(0),
                    then_arguments: vec![],
                    else_target: BlockId(1),
                    else_arguments: vec![],
                });
                body.blocks.push(returned);
            }
            let mut source = Module::new("frame_allocation");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            assert!(matches!(
                run(caller, callee, site, continuation),
                Err(CallInstanceEmissionErrorV1::CalleeFrameAllocation)
            ));
        }
    }

    #[test]
    fn splice_preserves_caller_frame_allocations() {
        let (mut caller, callee, site, continuation) = fixture();
        let scalar = Type::Scalar(ScalarType::U32);
        caller.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(
                    ValueId(4),
                    Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                ),
                OperationKind::Alloca {
                    element: scalar,
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ));
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("caller_frame");
        module.functions.push(result.caller);
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_preserves_workgroup_allocation_declaration_origins() {
        for in_callee in [true, false] {
            let (mut caller, mut callee, site, continuation) = fixture();
            let scalar = Type::Scalar(ScalarType::U32);
            let allocation = Operation::effect_free(
                ValueDef::new(
                    ValueId(if in_callee { 103 } else { 4 }),
                    Type::pointer(
                        scalar.clone(),
                        AddressSpace::Workgroup,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::WorkgroupMemory(fe2o3_kernel_ir::WorkgroupMemory {
                    element: scalar,
                    extent: fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(1),
                    alignment: 4,
                }),
            );
            let function = if in_callee { &mut callee } else { &mut caller };
            function.body.as_mut().unwrap().blocks[0]
                .operations
                .push(allocation);
            let mut source = Module::new("workgroup_allocation");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            let result = run(caller, callee, site, continuation);
            if in_callee {
                assert!(matches!(
                    result,
                    Err(CallInstanceEmissionErrorV1::CalleeWorkgroupAllocation)
                ));
            } else {
                let mut module = Module::new("caller_workgroup_allocation");
                module.functions.push(result.unwrap().caller);
                verify_module(&module).unwrap();
            }
        }
    }

    #[test]
    fn splice_requires_collective_occurrence_proof_but_preserves_local_effects() {
        use fe2o3_kernel_ir::{
            Barrier, BarrierSemantics, Convergence, Fence, IntrinsicOperation, MemoryOrdering,
            SynchronizationScope, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
        };
        let scope = SynchronizationScope::Workgroup;
        let semantics =
            BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]);
        for (kind, ty, collective) in [
            (
                OperationKind::Barrier(Barrier {
                    execution_scope: scope,
                    memory_scope: scope,
                    semantics: semantics.clone(),
                }),
                None,
                true,
            ),
            (
                OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                    memory_scope: scope,
                    semantics: semantics.clone(),
                    convergence: Convergence::uniform(scope),
                }),
                None,
                true,
            ),
            (
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
                Some(Type::Scalar(ScalarType::U32)),
                true,
            ),
            (
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                Some(Type::INDEX),
                false,
            ),
            (
                OperationKind::Fence(Fence {
                    memory_scope: scope,
                    semantics,
                }),
                None,
                false,
            ),
        ] {
            let (caller, mut callee, site, continuation) = fixture();
            let results = ty
                .map(|ty| vec![ValueDef::new(ValueId(103), ty)])
                .unwrap_or_default();
            callee.body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(results, kind));
            let mut source = Module::new("call_effects");
            source.functions = vec![caller.clone(), callee.clone()];
            verify_module(&source).unwrap();
            let result = run(caller, callee, site, continuation);
            if collective {
                assert!(matches!(
                    result,
                    Err(CallInstanceEmissionErrorV1::CalleeCollective)
                ));
            } else {
                let mut module = Module::new("local_effects");
                module.functions.push(result.unwrap().caller);
                verify_module(&module).unwrap();
            }
        }
    }

    #[test]
    fn splice_preserves_nested_retained_calls() {
        let (caller, mut callee, site, continuation) = fixture();
        let mut nested = callee.clone();
        nested.id = FunctionId::new("nested");
        callee.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(103), Type::Scalar(ScalarType::U32)),
                OperationKind::Call {
                    callee: nested.id.clone(),
                    arguments: vec![ValueId(100), ValueId(101)],
                },
            ));
        let mut source = Module::new("nested_source");
        source.functions = vec![caller.clone(), callee.clone(), nested.clone()];
        verify_module(&source).unwrap();
        let result = run(caller, callee, site, continuation).unwrap();
        let mut module = Module::new("nested_result");
        module.functions = vec![result.caller, nested];
        verify_module(&module).unwrap();
    }

    #[test]
    fn splice_budget_is_cumulative_and_preserves_the_owner_floor() {
        let measure = |work_limit, storage_limit| {
            let (caller, callee, site, continuation) = fixture();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(7).unwrap();
            let result = splice_production_call_instance_v1(
                caller,
                callee,
                site,
                BlockId(16),
                continuation,
                &mut budget,
            );
            (
                result,
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            )
        };
        let (result, exact_work, live, peak) = measure(1_000_000, 1_000_000);
        let result = result.unwrap();
        assert_eq!(live, 7 + result.additional_storage_bytes);
        assert!(peak >= live);
        assert!(measure(exact_work, peak).0.is_ok());
        let (failed, _, remaining, _) = measure(exact_work - 1, peak);
        assert!(matches!(
            failed,
            Err(CallInstanceEmissionErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
            ))
        ));
        assert_eq!(remaining, 7);
        let (failed, _, remaining, _) = measure(exact_work, peak - 1);
        assert!(matches!(
            failed,
            Err(CallInstanceEmissionErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
            ))
        ));
        assert_eq!(remaining, 7);
    }
