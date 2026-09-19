mod helper_value_route_negatives {
    use super::*;
    use fe2o3_kernel_ir::{Fence, VerifiedCanonicalKernelIrModuleV12};
    use native_helper_value_template_v1::{Ledger, Meter};

    struct RouteMeter<'a, 'w> {
        budget: &'a mut ArgumentBudgetV1<'w>,
        failed: bool,
    }
    impl Meter for RouteMeter<'_, '_> {
        fn work(&mut self, n: usize) -> Result<(), &'static str> {
            self.budget.charge_work(n).map_err(|_| {
                self.failed = true;
                "route work"
            })
        }
        fn reserve(&mut self, n: usize) -> Result<(), &'static str> {
            self.budget.reserve_storage(n).map_err(|_| {
                self.failed = true;
                "route storage"
            })
        }
        fn release(&mut self, n: usize) -> Result<(), &'static str> {
            self.budget
                .release_storage(n)
                .map_err(|_| "route accounting")
        }
        fn storage(&self) -> Result<usize, &'static str> {
            Ok(self.budget.storage())
        }
        fn exhausted(&self) -> bool {
            self.failed
        }
        fn identity(&mut self) -> Result<Ledger, &'static str> {
            Ok(Ledger {
                slot: self.budget as *const ArgumentBudgetV1<'_> as usize,
                work: self.budget.work_ledger_identity_v1(),
            })
        }
    }

    fn next_value(function: &Function) -> ValueId {
        let body = function.body.as_ref().unwrap();
        ValueId(
            body.parameters
                .iter()
                .map(|v| v.0)
                .chain(
                    body.blocks
                        .iter()
                        .flat_map(|b| &b.parameters)
                        .map(|v| v.id.0),
                )
                .chain(
                    body.blocks
                        .iter()
                        .flat_map(|b| &b.operations)
                        .flat_map(|op| &op.results)
                        .map(|v| v.id.0),
                )
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .unwrap(),
        )
    }

    fn check_changed_native(
        source: &ProductionPreRankedKirOwnerV1,
        module: &Module,
        lowering: &ProductionRankedKernelLoweringInputV1,
        expected_query: &'static str,
    ) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let (actual, retained) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                module,
                &mut budget,
            )
            .unwrap();
        budget
            .reserve_storage(19 + retained.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let module = actual.module();
        let entry = module
            .functions
            .iter()
            .find(|f| f.id == module.kernels[0].entry)
            .unwrap();
        let (location, operation) = entry
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, op)| {
                        matches!(op.kind, OperationKind::Call { .. })
                            .then_some((FunctionOperationLocation::new(block.id, ordinal), op))
                    })
            })
            .unwrap();
        native_helper_value_context_v1::with_native_helper_values(
            source.semantic_ssa().source_semantic(),
            module,
            &source.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            &mut RouteMeter {
                budget: &mut budget,
                failed: false,
            },
            |context, meter| {
                assert_eq!(
                    context.root_call(entry, location, operation, meter).err(),
                    Some(expected_query)
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let written = entry.body.as_ref().unwrap().blocks.iter().find_map(|block| {
            block.operations.iter().enumerate().find_map(|(ordinal, op)| {
                matches!(&op.kind, OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global)
                    .then_some(FunctionOperationLocation::new(block.id, ordinal))
            })
        }).unwrap();
        assert_eq!(
            validate_mir_pliron_translation_with_semantic_and_budget_v1(
                Some(source.semantic_ssa().source_semantic()),
                module,
                &source.correspondence,
                NAME,
                lowering,
                &access(0),
                &[],
                source.limits.max_operations,
                &mut budget,
            )
            .err(),
            Some(
                ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch {
                    location: written
                }
            ),
        );
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn verified_same_return_hidden_fence_and_extra_block_refuse_actual_global_valueaccess() {
        for extra_block in [false, true] {
            let source = source(false, 32, false, false);
            let lowering = ranked_wrapping_lowering(&source, false, 32, 1, RankedMutation::None);
            let mut module = source.executable().module().clone();
            let helper = module
                .functions
                .iter_mut()
                .find(|f| f.role == FunctionRole::InternalHelper)
                .unwrap();
            let fresh_value = next_value(helper);
            let body = helper.body.as_mut().unwrap();
            if extra_block {
                let id = BlockId(
                    body.blocks
                        .iter()
                        .map(|b| b.id.0)
                        .max()
                        .unwrap()
                        .checked_add(1)
                        .unwrap(),
                );
                let mut block = BasicBlock::new(id);
                block.operations.push(Operation::effect_free(
                    ValueDef::new(fresh_value, Type::Scalar(ScalarType::U32)),
                    OperationKind::Constant(Constant::U32(0)),
                ));
                block.terminator = Some(Terminator::Return {
                    values: vec![fresh_value],
                });
                body.blocks.push(block);
            } else {
                body.blocks[0].operations.push(Operation::new(
                    vec![],
                    OperationKind::Fence(Fence {
                        memory_scope: SynchronizationScope::Device,
                        semantics: BarrierSemantics::new(
                            MemoryOrdering::AcquireRelease,
                            [AddressSpace::Global],
                        ),
                    }),
                ));
            }
            check_changed_native(
                &source,
                &module,
                &lowering,
                "unresolved actual native helper return",
            );
        }
    }

    #[test]
    fn verified_extra_native_return_cannot_reuse_single_result_source_abi() {
        let source = source(false, 32, false, false);
        let lowering = ranked_wrapping_lowering(&source, false, 32, 1, RankedMutation::None);
        let mut module = source.executable().module().clone();
        let helper = module
            .functions
            .iter_mut()
            .find(|f| f.role == FunctionRole::InternalHelper)
            .unwrap();
        helper.signature.results.push(Type::Scalar(ScalarType::U32));
        for block in &mut helper.body.as_mut().unwrap().blocks {
            if let Some(Terminator::Return { values }) = &mut block.terminator {
                assert_eq!(values.len(), 1);
                values.push(values[0]);
            }
        }
        let entry_id = module.kernels[0].entry.clone();
        let entry = module
            .functions
            .iter_mut()
            .find(|f| f.id == entry_id)
            .unwrap();
        let fresh = next_value(entry);
        let call = entry
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.operations)
            .find(|op| matches!(op.kind, OperationKind::Call { .. }))
            .unwrap();
        assert_eq!(call.results.len(), 1);
        call.results
            .push(ValueDef::new(fresh, Type::Scalar(ScalarType::U32)));
        check_changed_native(
            &source,
            &module,
            &lowering,
            "native call callee, arity or result changed",
        );
    }
}
