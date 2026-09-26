// A borrowed projection of the original SSA plan, not source or emission
// authority. The owning emission relation must still bind actual ABI values,
// entry parameters, operations and control to this exact source instance.
const INVOCATION_ENTRY_RELATION_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InvocationBlockLayoutV1 {
    source_entry: BlockId,
    preheader: Option<BlockId>,
    trap: Option<BlockId>,
    first_block: u32,
    next_block: u32,
    entry_predecessors: usize,
}

struct InvocationEntryPlanV1<'source> {
    source: &'source SemanticFunctionDeclV1,
    ssa: &'source ProductionSemanticSsaFunctionPlanV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    layout: InvocationBlockLayoutV1,
}

fn invocation_entry_error_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "invocation entry differs from its original source SSA plan",
    )
}

impl InvocationEntryPlanV1<'_> {
    fn derive<'source>(
        source: &'source SemanticFunctionDeclV1,
        ssa: &'source ProductionSemanticSsaFunctionPlanV1,
        placement: SemanticEmissionPlacementV1,
        has_runtime_trap: bool,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<InvocationEntryPlanV1<'source>, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let entry = SsaBlockIdV1::new(source.entry().index());
        let plan = ssa.plan();
        if ssa.function_identity() != source.identity()
            || source.blocks().is_empty()
            || plan.reverse_postorder().first() != Some(&entry)
            || !plan.is_reachable(entry)
        {
            return Err(invocation_entry_error_v1());
        }
        let mut entry_predecessors = 0;
        for (index, block) in source.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let index = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            if !plan.is_reachable(SsaBlockIdV1::new(index)) {
                continue;
            }
            block.terminator().kind().try_for_each_edge(|edge| {
                budget.charge_work(2)?;
                if edge.target().index() as usize >= source.blocks().len()
                    || !plan.is_reachable(SsaBlockIdV1::new(edge.target().index()))
                {
                    return Err(invocation_entry_error_v1());
                }
                if edge.target() == source.entry() {
                    entry_predecessors = argument_sum_v1(&[entry_predecessors, 1])?;
                }
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
        }
        let arguments = plan.entry_arguments();
        let definitions = plan.entry_definitions();
        let variables = plan
            .transport_variables(entry)
            .ok_or_else(invocation_entry_error_v1)?;
        budget.charge_work(definitions.len())?;
        if arguments.len() != variables.len()
            || definitions
                .windows(2)
                .any(|pair| pair[0].variable() >= pair[1].variable())
            || entry_predecessors == 0 && !arguments.is_empty()
        {
            return Err(invocation_entry_error_v1());
        }
        let lookup = definitions.len().checked_ilog2().unwrap_or(0) as usize + 2;
        for (index, (argument, variable)) in arguments.iter().zip(variables).enumerate() {
            budget.charge_work(argument_sum_v1(&[lookup, 2])?)?;
            if argument.variable() != *variable
                || index != 0 && arguments[index - 1].variable() >= argument.variable()
            {
                return Err(invocation_entry_error_v1());
            }
            let definition = definitions
                .binary_search_by_key(&argument.variable(), |row| row.variable())
                .map_err(|_| invocation_entry_error_v1())?;
            if definitions[definition].value() != argument.value() {
                return Err(invocation_entry_error_v1());
            }
        }
        let count =
            u32::try_from(source.blocks().len()).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let trap = has_runtime_trap
            .then(|| placement.block(count))
            .transpose()?;
        let preheader = (entry_predecessors != 0)
            .then(|| {
                placement.block(
                    count
                        .checked_add(u32::from(has_runtime_trap))
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )
            })
            .transpose()?;
        let reserved = count
            .checked_add(u32::from(has_runtime_trap))
            .and_then(|count| count.checked_add(u32::from(preheader.is_some())))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let next_block = placement
            .first_block
            .checked_add(reserved)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(InvocationEntryPlanV1 {
            source,
            ssa,
            ledger: budget.work_ledger_identity_v1(),
            layout: InvocationBlockLayoutV1 {
                source_entry: placement.block(source.entry().index())?,
                preheader,
                trap,
                first_block: placement.first_block,
                next_block,
                entry_predecessors,
            },
        })
    }

    fn check_source(
        &self,
        source: &SemanticFunctionDeclV1,
        ssa: &ProductionSemanticSsaFunctionPlanV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || !std::ptr::eq(self.source, source)
            || !std::ptr::eq(self.ssa, ssa)
        {
            return Err(invocation_entry_error_v1());
        }
        Ok(())
    }

    fn check_arguments(
        &self,
        arguments: &[SsaArgumentV1],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_source(self.source, self.ssa, budget)?;
        budget.charge_work(argument_sum_v1(&[arguments.len(), 1])?)?;
        if self.ssa.plan().entry_arguments() != arguments {
            return Err(invocation_entry_error_v1());
        }
        Ok(())
    }

    fn entry_arguments(&self) -> &[SsaArgumentV1] {
        self.ssa.plan().entry_arguments()
    }

    fn invocation_block(&self) -> BlockId {
        self.layout.preheader.unwrap_or(self.layout.source_entry)
    }
}

include!("production_invocation_entry_resources_v1.rs");
