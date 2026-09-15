use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticConstantValueV1, SemanticDefinedCapabilityContractV1,
    SemanticReusableLdsConversionV1, SemanticUnwindActionV1,
};
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedStatementOriginV1 as S,
    SemanticExpandedTerminatorOriginV1 as T,
};
use std::mem::{size_of, size_of_val};

/// A replay-checked transfer of an existing allocation, not a new issuer. The
/// input/output logical identities retain the allocation's brand and epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticReusableLdsResultV1 {
    record: SemanticReusableLdsConversionV1,
    caller: SemanticCallInstanceIdV1,
    callee: SemanticCallInstanceIdV1,
    allocation_block: SemanticBlockIdV1,
    allocation: SemanticLocalIdV1,
    parameter_block: SemanticBlockIdV1,
    parameter_statement: u32,
    parameter: SemanticLocalIdV1,
    return_block: SemanticBlockIdV1,
    return_statement: u32,
    return_local: SemanticLocalIdV1,
    destination: SemanticLocalIdV1,
    erased: bool,
}

impl ProductionSemanticReusableLdsResultV1 {
    pub const fn record(&self) -> SemanticReusableLdsConversionV1 {
        self.record
    }
    pub const fn caller(&self) -> SemanticCallInstanceIdV1 {
        self.caller
    }
    pub const fn callee(&self) -> SemanticCallInstanceIdV1 {
        self.callee
    }
    pub const fn allocation_block(&self) -> SemanticBlockIdV1 {
        self.allocation_block
    }
    pub const fn allocation(&self) -> SemanticLocalIdV1 {
        self.allocation
    }
    pub const fn parameter_block(&self) -> SemanticBlockIdV1 {
        self.parameter_block
    }
    pub const fn parameter_statement(&self) -> u32 {
        self.parameter_statement
    }
    pub const fn parameter(&self) -> SemanticLocalIdV1 {
        self.parameter
    }
    pub const fn return_block(&self) -> SemanticBlockIdV1 {
        self.return_block
    }
    pub const fn return_statement(&self) -> u32 {
        self.return_statement
    }
    pub const fn return_local(&self) -> SemanticLocalIdV1 {
        self.return_local
    }
    pub const fn destination(&self) -> SemanticLocalIdV1 {
        self.destination
    }
    pub const fn erased(&self) -> bool {
        self.erased
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DefinedReusableLdsResultsV1 {
    view_identity: Option<[u8; 32]>,
    entries: Box<[ProductionSemanticReusableLdsResultV1]>,
    events: Box<[(SemanticBlockIdV1, u32, usize, bool)]>,
    resources: SemanticSsaAuxiliaryResourcesV1,
}

#[path = "defined_reusable_lds_results/frames.rs"]
mod frames;

fn mismatch() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::ReplayMismatch
}
fn assignment(
    body: &SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
    statement: u32,
) -> Result<&SemanticAssignmentV1, ProductionSemanticSsaErrorV1> {
    match body
        .blocks()
        .get(block.index() as usize)
        .and_then(|b| b.statements().get(statement as usize))
        .map(|s| s.kind())
    {
        Some(SemanticStatementKindV1::Assign(value)) => Ok(value),
        _ => Err(mismatch()),
    }
}

impl DefinedReusableLdsResultsV1 {
    pub(super) fn derive(
        semantic: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &SemanticExpandedRootV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        if !expansion
            .root(view.root())
            .is_some_and(|expected| std::ptr::eq(expected, view))
        {
            return Err(mismatch());
        }
        let mut resources = SemanticSsaAuxiliaryResourcesV1 {
            storage_words: 16,
            work_units: 0,
        };
        let mut charge = |work: usize, words: usize| -> Result<(), ProductionSemanticSsaErrorV1> {
            resources.work_units = resources
                .work_units
                .checked_add(work)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            resources.storage_words = resources
                .storage_words
                .checked_add(words)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            enforce_function_resource_limit_v1(view.source_body(), resources, limits)
        };
        charge(semantic.functions().len(), 0)?;
        if !semantic.functions().iter().any(|f| matches!(f.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(r)) if r.provenance().root() == view.root())) {
            return Ok(Self { resources, ..Self::default() });
        }
        let bindings = expansion
            .defined_capability_bindings(semantic)
            .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
        for binding in &bindings {
            let mut bytes = size_of_val(binding)
                .checked_add(size_of_val(binding.arguments()))
                .and_then(|n| n.checked_add(size_of_val(binding.callee_arguments())))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            for place in binding
                .arguments()
                .iter()
                .filter_map(|a| match a {
                    SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => Some(p),
                    _ => None,
                })
                .chain(std::iter::once(binding.destination()))
            {
                bytes = bytes
                    .checked_add(size_of_val(place.projections()))
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            }
            charge(
                binding.arguments().len() + 1,
                bytes
                    .div_ceil(size_of::<usize>())
                    .checked_mul(2)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            )?;
        }
        let mut entries = Vec::new();
        let mut events = Vec::new();
        for binding in &bindings {
            charge(1, 0)?;
            let SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record) =
                binding.contract()
            else {
                continue;
            };
            if binding.root() != view.root() {
                continue;
            }
            let source = record.source();
            if binding.root_identity() != view.identity()
                || binding.caller_function() != source.caller
                || binding.call_block() != source.conversion_block
                || binding.arguments().len() != 1
                || binding.callee_arguments().len() != 1
            {
                return Err(mismatch());
            }
            let mut allocation = None;
            let mut parameter = None;
            let mut result = None;
            for (index, origin) in view.block_origins().iter().enumerate() {
                charge(origin.statements().len() + 1, 0)?;
                let block = SemanticBlockIdV1::from_index(index as u32);
                if origin.instance() == binding.caller_instance()
                    && origin.function() == source.caller
                    && origin.block() == source.allocation_block
                {
                    if origin.terminator() != T::Source {
                        return Err(mismatch());
                    }
                    let SemanticTerminatorKindV1::Call(call) =
                        view.body().blocks()[index].terminator().kind()
                    else {
                        return Err(mismatch());
                    };
                    let destination = call.destination().ok_or_else(mismatch)?;
                    if call.callee() != source.allocation_callable
                        || call.unwind() != SemanticUnwindActionV1::Unreachable
                        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                        || destination.edge().target() != binding.expanded_call_block()
                        || destination.place().ty() != record.types().input
                        || !destination.place().projections().is_empty()
                    {
                        return Err(mismatch());
                    }
                    let local = destination.place().local();
                    let local_origin = view
                        .local_origins()
                        .get(local.index() as usize)
                        .ok_or_else(mismatch)?;
                    if local_origin.instance() != binding.caller_instance()
                        || local_origin.function() != source.caller
                        || local_origin.local() != source.allocation_local
                        || allocation.replace((block, local)).is_some()
                    {
                        return Err(mismatch());
                    }
                }
                for (statement, marker) in origin.statements().iter().enumerate() {
                    match *marker {
                        S::ParameterTransfer {
                            callee,
                            argument: 0,
                        } if callee == binding.callee_instance() => {
                            if origin.instance() != binding.caller_instance()
                                || block != binding.expanded_call_block()
                                || origin.terminator() != (T::CallEntry { callee })
                                || parameter.replace((block, statement as u32)).is_some()
                            {
                                return Err(mismatch());
                            }
                        }
                        S::ReturnTransfer { callee } if callee == binding.callee_instance() => {
                            if origin.instance() != callee
                                || origin.function() != record.function()
                                || result.replace((block, statement as u32)).is_some()
                            {
                                return Err(mismatch());
                            }
                        }
                        _ => {}
                    }
                }
            }
            let (allocation_block, allocation) = allocation.ok_or_else(mismatch)?;
            let (parameter_block, parameter_statement) = parameter.ok_or_else(mismatch)?;
            let (return_block, return_statement) = result.ok_or_else(mismatch)?;
            if return_block != binding.expanded_entry_block() {
                return Err(mismatch());
            }
            let value = assignment(view.body(), parameter_block, parameter_statement)?;
            if value.destination().local() != binding.callee_arguments()[0]
                || value.destination().ty() != record.types().input
                || !value.destination().projections().is_empty()
            {
                return Err(mismatch());
            }
            let erased = match value.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(c)) => {
                    c.ty() == record.types().input
                        && matches!(c.value(), SemanticConstantValueV1::ZeroSized)
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local() == allocation
                        && p.ty() == record.types().input
                        && p.projections().is_empty() =>
                {
                    false
                }
                _ => return Err(mismatch()),
            };
            if !erased
                && !matches!(
                    value.value().kind(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_))
                )
            {
                return Err(mismatch());
            }
            // The empty source still has the expander's exact frame lifetime
            // markers. They kill storage, never define capability authority.
            charge(8, 0)?;
            frames::origins(
                view,
                binding,
                (parameter_block, parameter_statement),
                (return_block, return_statement),
            )?;
            let transferred = assignment(view.body(), return_block, return_statement)?;
            if transferred.destination() != binding.destination()
                || !matches!(transferred.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local() == binding.callee_return() && p.projections().is_empty() && p.ty() == record.types().output)
            {
                return Err(mismatch());
            }
            for (local, role) in [
                (
                    binding.callee_arguments()[0],
                    SemanticLocalRoleV1::Argument(0),
                ),
                (binding.callee_return(), SemanticLocalRoleV1::Return),
            ] {
                let origin = view
                    .local_origins()
                    .get(local.index() as usize)
                    .ok_or_else(mismatch)?;
                if origin.instance() != binding.callee_instance()
                    || origin.function() != record.function()
                    || semantic.functions()[record.function().index() as usize]
                        .locals()
                        .get(origin.local().index() as usize)
                        .is_none_or(|local| local.role() != role)
                {
                    return Err(mismatch());
                }
            }
            // Check actual expanded predecessors, including otherwise dead
            // edges. The producer's CallReturn is the sole legal entry.
            let mut incoming = 0;
            for (index, block) in view.body().blocks().iter().enumerate() {
                charge(1, 0)?;
                block
                    .terminator()
                    .kind()
                    .try_for_each_edge::<ProductionSemanticSsaErrorV1>(|edge| {
                        charge(1, 0)?;
                        if edge.target() == parameter_block {
                            if index != allocation_block.index() as usize
                                || edge.role() != SemanticEdgeRoleV1::CallReturn
                            {
                                return Err(mismatch());
                            }
                            incoming += 1;
                        }
                        Ok(())
                    })?;
            }
            if incoming != 1 || parameter_block == view.body().entry() {
                return Err(mismatch());
            }
            // Charge both Vec growth buffers plus the final boxed rows/event
            // index. Bindings stay alive throughout, included above.
            let words = (size_of::<ProductionSemanticReusableLdsResultV1>()
                + 2 * size_of::<(SemanticBlockIdV1, u32, usize, bool)>())
            .div_ceil(size_of::<usize>())
            .checked_mul(4)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            charge(64, words)?;
            let index = entries.len();
            events.push((parameter_block, parameter_statement, index, false));
            events.push((return_block, return_statement, index, true));
            entries.push(ProductionSemanticReusableLdsResultV1 {
                record,
                caller: binding.caller_instance(),
                callee: binding.callee_instance(),
                allocation_block,
                allocation,
                parameter_block,
                parameter_statement,
                parameter: binding.callee_arguments()[0],
                return_block,
                return_statement,
                return_local: binding.callee_return(),
                destination: binding.destination().local(),
                erased,
            });
        }
        charge(
            events
                .len()
                .checked_mul((usize::BITS - events.len().leading_zeros()) as usize + 1)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            0,
        )?;
        events.sort_unstable_by_key(|row| (row.0, row.1));
        if events
            .windows(2)
            .any(|pair| (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1))
        {
            return Err(mismatch());
        }
        let output = Self {
            view_identity: Some(*view.identity()),
            entries: entries.into_boxed_slice(),
            events: events.into_boxed_slice(),
            resources,
        };
        output.verify_markers(view.body())?;
        Ok(output)
    }

    pub(super) fn entries(&self) -> &[ProductionSemanticReusableLdsResultV1] {
        &self.entries
    }
    pub(super) const fn resources(&self) -> SemanticSsaAuxiliaryResourcesV1 {
        self.resources
    }
    pub(super) fn verify_view(
        &self,
        view: &SemanticExpandedRootV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.entries.is_empty() && self.view_identity.is_none() {
            return Ok(());
        }
        if self.view_identity != Some(*view.identity()) {
            return Err(mismatch());
        }
        self.verify_markers(view.body())
    }
    pub(super) fn verify_markers(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        for entry in &self.entries {
            frames::statements(function, entry)?;
            let parameter = assignment(function, entry.parameter_block, entry.parameter_statement)?;
            let output = assignment(function, entry.return_block, entry.return_statement)?;
            let input = entry.record.types().input;
            let result = entry.record.types().output;
            if parameter.destination().local() != entry.parameter
                || parameter.destination().ty() != input
                || !parameter.destination().projections().is_empty()
                || if entry.erased {
                    !matches!(parameter.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(c))
                    if c.ty() == input && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
                } else {
                    !matches!(parameter.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local() == entry.allocation && p.ty() == input && p.projections().is_empty())
                }
                || output.destination().local() != entry.destination
                || output.destination().ty() != result
                || !output.destination().projections().is_empty()
                || !matches!(output.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local() == entry.return_local && p.ty() == result && p.projections().is_empty())
            {
                return Err(mismatch());
            }
        }
        Ok(())
    }

    /// Preserve the original transfer events. Erased input custody consumes the
    /// exact allocation immediately before the original parameter definition;
    /// the return consumes that parameter before the original return move.
    pub(super) fn append_result_events(
        &self,
        block: u32,
        statement: u32,
        events: &mut Vec<SsaEventV1>,
    ) {
        if let Ok(index) = self
            .events
            .binary_search_by_key(&(SemanticBlockIdV1::from_index(block), statement), |row| {
                (row.0, row.1)
            })
        {
            let (_, _, entry, returning) = self.events[index];
            let entry = self.entries[entry];
            if returning {
                events.push(SsaEventV1::Use(SsaVariableIdV1::new(
                    entry.parameter.index(),
                )));
                events.push(SsaEventV1::Kill(SsaVariableIdV1::new(
                    entry.parameter.index(),
                )));
                events.push(SsaEventV1::Define(SsaVariableIdV1::new(
                    entry.return_local.index(),
                )));
            } else if entry.erased {
                events.push(SsaEventV1::Use(SsaVariableIdV1::new(
                    entry.allocation.index(),
                )));
                events.push(SsaEventV1::Kill(SsaVariableIdV1::new(
                    entry.allocation.index(),
                )));
            }
        }
    }
    pub(super) fn hash_into(&self, digest: &mut Sha256) {
        if self.entries.is_empty() {
            return;
        }
        digest.update(b"fe2o3.execution-reusable-lds-transfer.v1\0");
        digest.update(self.view_identity.expect("checked transfer has a view"));
        digest.update((self.entries.len() as u64).to_le_bytes());
        for entry in &self.entries {
            digest.update(entry.record.body_identity());
            digest.update(entry.record.source().source_binding);
            for value in [
                entry.record.function().index(),
                entry.caller.index(),
                entry.callee.index(),
                entry.allocation_block.index(),
                entry.allocation.index(),
                entry.parameter_block.index(),
                entry.parameter_statement,
                entry.parameter.index(),
                entry.return_block.index(),
                entry.return_statement,
                entry.return_local.index(),
                entry.destination.index(),
            ] {
                digest.update(value.to_le_bytes());
            }
            digest.update([u8::from(entry.erased)]);
        }
    }
}

#[cfg(test)]
#[path = "defined_reusable_lds_results/tests.rs"]
mod tests;
