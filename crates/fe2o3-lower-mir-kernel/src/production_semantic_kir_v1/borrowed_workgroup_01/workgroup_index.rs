// Workgroup index issuance uses the same replayed shared-loan resolver.

impl BorrowedWorkgroupPlanV1<'_> {
    fn shared_consumer_receiver(
        &mut self,
        block: SemanticBlockIdV1,
        expected: SemanticExecutionCapabilityContractV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
    ) -> Result<(ValueId, Type)> {
        if lowering.correspondence_owner != self.view.root()
            || lowering.function != execution_function_for_root_v1(self.owner, self.view.root())?
            || lowering.kernel_context != Some(self.context)
            || lowering.execution_expansion_identity
                != self
                    .view
                    .has_expanded_calls()
                    .then(|| *self.view.identity())
            || lowering.execution_source_expansion_identity
                != execution_expansion_identity_v1(self.owner)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let (call, contract) = self.execution_call(block.index())?;
        let output = match contract.operation() {
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 { option, .. } => option,
            SemanticExecutionCapabilityOperationV1::LdsAllocate { lds, .. } => {
                let callable = self.owner.source_semantic().callables()
                    .get(call.callee().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let Some((checked, _)) = allocation_callable_pair(lowering.types, callable)? else {
                    return Err(reject("LDS allocation lost its exact borrowed source callable"));
                };
                if checked != contract {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                lds
            }
            _ => return Err(reject("expected scoped index or LDS allocation receiver")),
        };
        let (workgroup_reference, workgroup) = workgroup_reference_pair(lowering.types, contract)?;
        if contract != expected
            || contract.epoch_after().is_some()
            || !contract.signature().arguments().eq([workgroup_reference])
            || contract.signature().output() != output
            || !shared_type(lowering.types, workgroup_reference, workgroup)
        {
            return Err(reject("scoped index source signature or epoch changed"));
        }
        let [receiver] = call.arguments() else {
            return Err(reject("scoped index receiver arity"));
        };
        let receiver = receiver.clone();
        let origin = self.resolve_operand(block.index(), &receiver, &[], contract, 0)?;
        if origin.loans.is_empty() || origin.epoch_projection {
            return Err(reject("scoped index receiver lost its exact shared loan"));
        }
        self.check_live(&origin, block.index())?;
        let values = lowering
            .semantic_ssa_bindings
            .get(&origin.issuer)
            .ok_or_else(|| reject("scoped index Workgroup issuer has not been lowered"))?
            .values()
            .map_err(reject)?;
        let [(owned_value, Type::ExecutionCapability(owned))] = values.as_slice() else {
            return Err(reject(
                "scoped index Workgroup issuer is not one capability",
            ));
        };
        if !owned.is_complete()
            || owned.role != ExecutionCapabilityRoleV1::Workgroup
            || owned.source_type != execution_type_identity_v1(lowering.types, workgroup)?
            || !same_provenance(owned, contract, self.context)
        {
            return Err(reject(
                "scoped index Workgroup issuer type or provenance changed",
            ));
        }
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = receiver else {
            return Err(reject("scoped index receiver is a constant"));
        };
        let live = lowering
            .locals
            .get(place.local().index() as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| reject("scoped index receiver is not live"))?
            .values()
            .map_err(reject)?;
        if !place.projections().is_empty() || live != values {
            return Err(reject(
                "scoped index receiver changed its exact Workgroup SSA value",
            ));
        }
        Ok((*owned_value, Type::ExecutionCapability(owned.clone())))
    }
}

impl SemanticFunctionLoweringV1<'_> {
    pub(super) fn lower_workgroup_index_receiver_v1(
        &mut self,
        block: SemanticBlockIdV1,
        contract: SemanticExecutionCapabilityContractV1,
    ) -> Result<(ValueId, Type)> {
        let mut plan = self
            .borrowed_workgroup
            .take()
            .ok_or_else(|| reject("scoped index lacks the replay-checked Workgroup SSA owner"))?;
        let result = plan.shared_consumer_receiver(block, contract, self);
        self.borrowed_workgroup = Some(plan);
        result
    }
}
