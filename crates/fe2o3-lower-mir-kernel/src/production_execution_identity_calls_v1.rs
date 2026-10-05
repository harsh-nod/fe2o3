impl ExecutionIdentitySourceIndexV1<'_, '_> {
    fn call_equations(
        &self,
        row: &ExecutionIdentitySourceRowV1,
        ordinal: usize,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        let instance = self
            .instances
            .instance(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let occurrences = self
            .instances
            .occurrences(row.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let definition = occurrences
            .edge_definitions()
            .get(ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        let edge = definition.edge();
        if definition.value() != row.value
            || definition.variable().get() != row.local.index()
            || !definition.is_reachable()
            || !definition.is_promoted()
        {
            return Err(execution_identity_error_v1());
        }
        let SemanticTerminatorKindV1::Call(source) = instance
            .declaration()
            .blocks()
            .get(edge.source().get() as usize)
            .ok_or_else(execution_identity_error_v1)?
            .terminator()
            .kind()
        else {
            return Err(execution_identity_error_v1());
        };
        let destination = source
            .destination()
            .ok_or_else(execution_identity_error_v1)?;
        if destination.place().local() != row.local
            || !destination.place().projections().is_empty()
            || destination.place().ty() != row.selection.ty
        {
            return Err(execution_identity_error_v1());
        }
        charge_execution_cfg_lookup_v29(self.calls.len(), budget)?;
        let index = *self
            .calls
            .get(&(row.instance.index(), edge.source().get()))
            .ok_or_else(execution_identity_error_v1)?;
        let call = self
            .instances
            .calls(row.instance)
            .and_then(|calls| calls.get(index))
            .ok_or_else(execution_identity_error_v1)?;
        if !std::ptr::eq(call.source(), source) {
            return Err(execution_identity_error_v1());
        }
        match (call.child(), call.callable()) {
            (Some(child), SemanticCallableDeclV1::Defined { function }) => {
                let child_row = self
                    .instances
                    .instance(child)
                    .ok_or_else(execution_identity_error_v1)?;
                if child_row.function() != *function
                    || self
                        .instances
                        .incoming(child)
                        .is_none_or(|incoming| !std::ptr::eq(incoming, call))
                {
                    return Err(execution_identity_error_v1());
                }
                self.return_equations(child, row.selection, output, budget)
            }
            (
                None,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(operation),
                    ..
                },
            ) => {
                use fe2o3_mir_model::semantic_mir_v1::SemanticExecutionOperationV29 as Op;
                let result_type = match *operation {
                    Op::ContextIssue { context } => context,
                    Op::WorkgroupDerive { workgroup, .. } => workgroup,
                    Op::MaskedTileLoadU32 { tile, .. } => tile,
                    Op::MaskedTileIntoFragmentU32 { fragment, .. } => fragment,
                    Op::LaneFragmentIntoPartsU32 { parts, .. } => parts,
                };
                let types = self.instances.owner().source_semantic().types();
                if result_type != row.selection.ty
                    || row.selection.count != 1
                    || execution_cfg_nominal_kind_v29(types, result_type)? != Some(false)
                {
                    return Err(execution_identity_error_v1());
                }
                budget.reserve_storage(std::mem::size_of::<Vec<usize>>())?;
                let mut dependencies = Vec::new();
                for (ordinal, operand) in source.arguments().iter().enumerate() {
                    budget.charge_work(1)?;
                    if execution_identity_channels_v1(types, operand.ty(), budget)? == 0 {
                        continue;
                    }
                    let source = self.operand(
                        row.instance,
                        ExecutionSiteV29::Terminator {
                            block: edge.source(),
                        },
                        ExecutionOperandV29::CallArgument(
                            u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        operand,
                        budget,
                    )?;
                    for component in 0..source.count {
                        emission_push_v1(
                            &mut dependencies,
                            argument_sum_v1(&[source.first, component])?,
                            budget,
                        )?;
                    }
                }
                // Only the authenticated source occurrence creates an origin.
                // Actual role/producer/borrow authority still comes from the
                // original lowering observer, not this identity equation.
                output.push(
                    ExecutionIdentityEquationKindV1::Producer,
                    &dependencies,
                    budget,
                )?;
                let scratch = argument_sum_v1(&[
                    std::mem::size_of::<Vec<usize>>(),
                    argument_product_v1(dependencies.capacity(), std::mem::size_of::<usize>())?,
                ])?;
                drop(dependencies);
                budget.release_storage(scratch)?;
                Ok(())
            }
            _ => Err(execution_identity_error_v1()),
        }
    }

    fn return_equations(
        &self,
        child: ProductionCallInstanceIdV1,
        destination: ExecutionIdentitySelectionV1,
        output: &mut ExecutionIdentityEquationsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let exits = self
            .instances
            .exits(child)
            .ok_or_else(execution_identity_error_v1)?;
        budget.reserve_storage(std::mem::size_of::<Vec<usize>>())?;
        let mut dependencies = emission_vec_v1(exits.len(), budget)?;
        for component in 0..destination.count {
            budget.charge_work(dependencies.len())?;
            dependencies.clear();
            for exit in exits {
                budget.charge_work(2)?;
                let production_call_instances_v1::ProductionInstanceExitKindV1::Return { local } =
                    exit.kind
                else {
                    continue;
                };
                if exit.instance != child {
                    return Err(execution_identity_error_v1());
                }
                let source = self.source_use(
                    child,
                    ExecutionSiteV29::Terminator {
                        block: SsaBlockIdV1::new(exit.block.index()),
                    },
                    ExecutionOperandV29::ReturnValue,
                    local,
                    budget,
                )?;
                if source.ty != destination.ty || source.count != destination.count {
                    return Err(execution_identity_error_v1());
                }
                dependencies.push(argument_sum_v1(&[source.first, component])?);
            }
            output.push(
                ExecutionIdentityEquationKindV1::Merge,
                &dependencies,
                budget,
            )?;
        }
        let scratch = argument_sum_v1(&[
            std::mem::size_of::<Vec<usize>>(),
            argument_product_v1(dependencies.capacity(), std::mem::size_of::<usize>())?,
        ])?;
        drop(dependencies);
        budget.release_storage(scratch)?;
        Ok(())
    }
}
