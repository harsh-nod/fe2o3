//! Checked occurrence transport shared by closed defined-capability contracts.

use super::*;

/// Original source metadata plus replay-checked call coordinates, not SSA proof.
/// Only the expansion owner can construct a binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticExpandedDefinedCapabilityV1 {
    contract: SemanticDefinedCapabilityContractV1,
    expansion_identity: [u8; 32],
    root_identity: [u8; 32],
    root: SemanticFunctionIdV1,
    caller_instance: SemanticCallInstanceIdV1,
    callee_instance: SemanticCallInstanceIdV1,
    caller_function: SemanticFunctionIdV1,
    call_block: SemanticBlockIdV1,
    expanded_call_block: SemanticBlockIdV1,
    expanded_entry_block: SemanticBlockIdV1,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
    callee_arguments: Vec<SemanticLocalIdV1>,
    callee_return: SemanticLocalIdV1,
}

impl SemanticExpandedDefinedCapabilityV1 {
    pub const fn contract(&self) -> SemanticDefinedCapabilityContractV1 {
        self.contract
    }
    pub const fn expansion_identity(&self) -> &[u8; 32] {
        &self.expansion_identity
    }
    pub const fn root_identity(&self) -> &[u8; 32] {
        &self.root_identity
    }
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    pub const fn caller_instance(&self) -> SemanticCallInstanceIdV1 {
        self.caller_instance
    }
    pub const fn callee_instance(&self) -> SemanticCallInstanceIdV1 {
        self.callee_instance
    }
    pub const fn caller_function(&self) -> SemanticFunctionIdV1 {
        self.caller_function
    }
    pub const fn call_block(&self) -> SemanticBlockIdV1 {
        self.call_block
    }
    pub const fn expanded_call_block(&self) -> SemanticBlockIdV1 {
        self.expanded_call_block
    }
    pub const fn expanded_entry_block(&self) -> SemanticBlockIdV1 {
        self.expanded_entry_block
    }
    pub fn arguments(&self) -> &[SemanticOperandV1] {
        &self.arguments
    }
    pub const fn destination(&self) -> &SemanticPlaceV1 {
        &self.destination
    }
    pub fn callee_arguments(&self) -> &[SemanticLocalIdV1] {
        &self.callee_arguments
    }
    /// Actual retained nested allocation sizes, excluding this row's inline
    /// size. Storage accounting only; this does not grant source/SSA authority.
    pub fn retained_auxiliary_bytes(&self) -> Option<usize> {
        let mut bytes = self.arguments.capacity().checked_mul(std::mem::size_of::<SemanticOperandV1>())?
            .checked_add(self.callee_arguments.capacity().checked_mul(std::mem::size_of::<SemanticLocalIdV1>())?)?
            .checked_add(std::mem::size_of_val(self.destination.projections()))?;
        for operand in &self.arguments {
            if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = operand {
                bytes = bytes.checked_add(std::mem::size_of_val(p.projections()))?;
            }
        }
        Some(bytes)
    }
    pub const fn callee_return(&self) -> SemanticLocalIdV1 {
        self.callee_return
    }
}

impl SemanticCallExpansionV1 {
    /// Replay binds every coordinate to the unchanged admitted source. Consumers
    /// must still resolve parameter transfers and actual dominating producers.
    pub fn defined_capability_bindings(
        &self,
        source: &AdmittedInertSemanticMirV1,
    ) -> Result<Vec<SemanticExpandedDefinedCapabilityV1>> {
        self.verify_replay(source)?;
        let mut budget = Budget::new(self.limits)?;
        let mut bindings = Vec::new();
        budget.charge(SemanticCallExpansionResourceV1::Roots, self.roots.len())?;
        for root in &self.roots {
            budget.charge(
                SemanticCallExpansionResourceV1::Instances,
                root.instances.len(),
            )?;
            for (index, callee) in root.instances.iter().enumerate() {
                budget.work(1)?;
                let body = &source.functions()[callee.function.index() as usize];
                let Some(contract) = body.defined_capability_contract().copied() else {
                    continue;
                };
                let unsupported = || SemanticCallExpansionErrorV1::Unsupported {
                    function: callee.function,
                    block: callee.call_block,
                    reason: "defined capability lacks exact source caller custody",
                };
                if contract.function() != callee.function
                    || contract.provenance().root() != root.root
                {
                    return Err(unsupported());
                }
                let parent = callee.parent.ok_or_else(unsupported)?;
                let caller = root
                    .instances
                    .get(parent.0 as usize)
                    .ok_or_else(unsupported)?;
                let call_block = callee.call_block.ok_or_else(unsupported)?;
                let source_caller = &source.functions()[caller.function.index() as usize];
                let source_block = source_caller
                    .blocks()
                    .get(call_block.index() as usize)
                    .ok_or_else(unsupported)?;
                let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                    return Err(unsupported());
                };
                if source.callables().get(call.callee().index() as usize)
                    != Some(&SemanticCallableDeclV1::Defined {
                        function: callee.function,
                    })
                    || call.arguments().len() != body.abi().source_input_types().len()
                {
                    return Err(unsupported());
                }
                let destination = call.destination().ok_or_else(unsupported)?;
                budget.work(body.locals().len() + call.arguments().len())?;
                let mut callee_arguments = vec![None; call.arguments().len()];
                let mut callee_return = None;
                for (local_index, local) in body.locals().iter().enumerate() {
                    let expanded =
                        remap::local(SemanticLocalIdV1::from_index(local_index as u32), callee);
                    match local.role() {
                        SemanticLocalRoleV1::Argument(argument) => {
                            let slot = callee_arguments
                                .get_mut(argument as usize)
                                .ok_or_else(unsupported)?;
                            if slot.replace(expanded).is_some() {
                                return Err(unsupported());
                            }
                        }
                        SemanticLocalRoleV1::Return => {
                            if callee_return.replace(expanded).is_some() {
                                return Err(unsupported());
                            }
                        }
                        SemanticLocalRoleV1::Temporary => {}
                    }
                }
                let callee_arguments = callee_arguments
                    .into_iter()
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(unsupported)?;
                let arguments = call
                    .arguments()
                    .iter()
                    .zip(body.abi().source_input_types())
                    .map(|(argument, expected)| {
                        let erased_reuse = matches!(contract,
                            SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)
                                if record.source().caller == caller.function
                                    && record.source().conversion_block == call_block
                                    && record.types().input == *expected
                                    && call.arguments().len() == 1)
                            && matches!(argument, SemanticOperandV1::Constant(c)
                                if c.ty() == *expected && matches!(c.value(), SemanticConstantValueV1::ZeroSized));
                        if argument.ty() != *expected
                            || (matches!(argument, SemanticOperandV1::Constant(_)) && !erased_reuse)
                        {
                            return Err(unsupported());
                        }
                        remap::operand(argument, caller, &mut budget)
                    })
                    .collect::<Result<Vec<_>>>()?;
                // Reuse the existing remapper, including projection-index locals.
                let SemanticOperandV1::Copy(destination) = remap::operand(
                    &SemanticOperandV1::Copy(destination.place().clone()),
                    caller,
                    &mut budget,
                )?
                else {
                    return Err(unsupported());
                };
                bindings.try_reserve(1).map_err(|_| {
                    SemanticCallExpansionErrorV1::Limit(SemanticCallExpansionResourceV1::Instances)
                })?;
                bindings.push(SemanticExpandedDefinedCapabilityV1 {
                    contract,
                    expansion_identity: self.identity,
                    root_identity: root.identity,
                    root: root.root,
                    caller_instance: parent,
                    callee_instance: SemanticCallInstanceIdV1(index as u32),
                    caller_function: caller.function,
                    call_block,
                    expanded_call_block: remap::block(call_block, caller),
                    expanded_entry_block: remap::block(body.entry(), callee),
                    arguments,
                    destination,
                    callee_arguments,
                    callee_return: callee_return.ok_or_else(unsupported)?,
                });
            }
        }
        Ok(bindings)
    }
}
