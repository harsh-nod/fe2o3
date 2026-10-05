mod execution_source_control_v29 {
    use super::*;
    use production_call_instances_v1::ProductionCallControlV1;

    struct BlockControlV29 {
        reachable: bool,
        call: Option<ProductionCallControlV1>,
    }

    // A paid projection of original control facts, not a second CFG analysis.
    pub(super) struct ExecutionSourceControlV29<'source> {
        source: ExecutionCallSourceV29,
        function: &'source SemanticFunctionDeclV1,
        ssa: &'source ProductionSemanticSsaFunctionPlanV1,
        instance: ProductionCallInstanceIdV1,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        blocks: Vec<BlockControlV29>,
        may_return: bool,
    }

    impl<'source> ExecutionSourceControlV29<'source> {
        pub(super) fn new(
            instances: &ExecutionInstancesV29<'source>,
            instance: ProductionCallInstanceIdV1,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            budget.charge_work(4)?;
            if instances.instance_reachable(instance) != Some(true) {
                return Err(execution_availability_error_v29());
            }
            let original = instances
                .instance(instance)
                .ok_or_else(execution_availability_error_v29)?;
            let function = original.declaration();
            let ssa = original.ssa();
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Self>(),
                std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            ])?)?;
            let mut blocks = emission_vec_v1(function.blocks().len(), budget)?;
            for (index, block) in function.blocks().iter().enumerate() {
                budget.charge_work(6)?;
                let id = SemanticBlockIdV1::from_index(
                    u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                let reachable = instances
                    .block_reachable(instance, id)
                    .ok_or_else(execution_availability_error_v29)?;
                let ssa_reachable = ssa.plan().is_reachable(SsaBlockIdV1::new(id.index()));
                if reachable && !ssa_reachable {
                    return Err(execution_availability_error_v29());
                }
                let call = if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(_))
                {
                    let call = if ssa_reachable {
                        instances
                            .call_control(ProductionCallOccurrenceV1 {
                                caller: instance,
                                block: id,
                            })
                            .ok_or_else(execution_availability_error_v29)?
                    } else {
                        // Syntactically dead calls have no instantiated call row.
                        ProductionCallControlV1::Unreachable
                    };
                    if reachable == (call == ProductionCallControlV1::Unreachable) {
                        return Err(execution_availability_error_v29());
                    }
                    Some(call)
                } else {
                    None
                };
                blocks.push(BlockControlV29 { reachable, call });
            }
            if !blocks
                .get(function.entry().index() as usize)
                .is_some_and(|block| block.reachable)
            {
                return Err(execution_availability_error_v29());
            }
            Ok(Self {
                source: ExecutionCallSourceV29::from_instances(instances, budget)?,
                function,
                ssa,
                instance,
                ledger: budget.work_ledger_identity_v1(),
                blocks,
                may_return: instances
                    .instance_may_return(instance)
                    .ok_or_else(execution_availability_error_v29)?,
            })
        }

        pub(super) fn check_source(
            &self,
            source: ExecutionCallSourceV29,
            function: &SemanticFunctionDeclV1,
            ssa: &ProductionSemanticSsaFunctionPlanV1,
            instance: ProductionCallInstanceIdV1,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            budget.charge_work(24)?;
            if self.source != source
                || !std::ptr::eq(self.function, function)
                || !std::ptr::eq(self.ssa, ssa)
                || self.instance != instance
                || self.ledger != budget.work_ledger_identity_v1()
            {
                return Err(execution_availability_error_v29());
            }
            Ok(())
        }

        pub(super) fn block_reachable(
            &self,
            block: SemanticBlockIdV1,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            budget.charge_work(2)?;
            if self.ledger != budget.work_ledger_identity_v1() {
                return Err(execution_availability_error_v29());
            }
            self.blocks
                .get(block.index() as usize)
                .map(|block| block.reachable)
                .ok_or_else(execution_availability_error_v29)
        }

        pub(super) fn call_control(
            &self,
            block: SemanticBlockIdV1,
            call: &SemanticDirectCallV1,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<ProductionCallControlV1, ProductionSemanticKirErrorV1> {
            budget.charge_work(4)?;
            if self.ledger != budget.work_ledger_identity_v1() {
                return Err(execution_availability_error_v29());
            }
            let source = self
                .function
                .blocks()
                .get(block.index() as usize)
                .ok_or_else(execution_availability_error_v29)?;
            let SemanticTerminatorKindV1::Call(original) = source.terminator().kind() else {
                return Err(execution_availability_error_v29());
            };
            if !std::ptr::eq(original, call) {
                return Err(execution_availability_error_v29());
            }
            self.blocks
                .get(block.index() as usize)
                .and_then(|block| block.call)
                .ok_or_else(execution_availability_error_v29)
        }

        pub(super) fn successor_reachable(
            &self,
            block: SemanticBlockIdV1,
            role: SemanticEdgeRoleV1,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            if !self.block_reachable(block, budget)? {
                return Ok(false);
            }
            if role != SemanticEdgeRoleV1::CallReturn {
                return Ok(true);
            }
            let SemanticTerminatorKindV1::Call(call) = self.function.blocks()
                [block.index() as usize]
                .terminator()
                .kind()
            else {
                return Err(execution_availability_error_v29());
            };
            match self.call_control(block, call, budget)? {
                ProductionCallControlV1::MayReturn => Ok(true),
                ProductionCallControlV1::NoNormalReturn => Ok(false),
                ProductionCallControlV1::Unreachable => Err(execution_availability_error_v29()),
            }
        }

        pub(super) fn may_return(
            &self,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            budget.charge_work(1)?;
            if self.ledger != budget.work_ledger_identity_v1() {
                return Err(execution_availability_error_v29());
            }
            Ok(self.may_return)
        }
    }
}

use execution_source_control_v29::ExecutionSourceControlV29;

impl ExecutionAvailabilityV29<'_> {
    fn source_block_reachable_v29(
        &self,
        block: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.control
            .check_source(self.source, self.function, self.ssa, self.instance, budget)?;
        self.control.block_reachable(block, budget)
    }

    fn source_call_control_v29(
        &self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<production_call_instances_v1::ProductionCallControlV1, ProductionSemanticKirErrorV1>
    {
        self.control
            .check_source(self.source, self.function, self.ssa, self.instance, budget)?;
        self.control.call_control(block, call, budget)
    }
}
