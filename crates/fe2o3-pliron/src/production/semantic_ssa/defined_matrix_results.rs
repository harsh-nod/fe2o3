use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssignmentV1, SemanticDefinedCapabilityContractV1};
use fe2o3_mir_model::{SemanticCallInstanceIdV1, SemanticExpandedStatementOriginV1};
use std::mem::{size_of, size_of_val};

/// A closed original getter/bridge result, not a Matrix issuer or ambient ZST.
/// Only replay of the complete Defined call chain can construct this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticMatrixBridgeResultV1 {
    getter: SemanticCallInstanceIdV1,
    bridge: SemanticCallInstanceIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
    getter_block: SemanticBlockIdV1,
    getter_statement: u32,
    return_local: SemanticLocalIdV1,
    destination: SemanticLocalIdV1,
    receiver: SemanticLocalIdV1,
    current: SemanticLocalIdV1,
    matrix: SemanticTypeIdV1,
}

impl ProductionSemanticMatrixBridgeResultV1 {
    pub const fn getter(&self) -> SemanticCallInstanceIdV1 {
        self.getter
    }
    pub const fn bridge(&self) -> SemanticCallInstanceIdV1 {
        self.bridge
    }
    pub const fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub const fn statement(&self) -> u32 {
        self.statement
    }
    pub const fn getter_block(&self) -> SemanticBlockIdV1 {
        self.getter_block
    }
    pub const fn getter_statement(&self) -> u32 {
        self.getter_statement
    }
    pub const fn return_local(&self) -> SemanticLocalIdV1 {
        self.return_local
    }
    pub const fn destination(&self) -> SemanticLocalIdV1 {
        self.destination
    }
    pub const fn receiver(&self) -> SemanticLocalIdV1 {
        self.receiver
    }
    pub const fn current(&self) -> SemanticLocalIdV1 {
        self.current
    }
    pub const fn matrix(&self) -> SemanticTypeIdV1 {
        self.matrix
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DefinedMatrixResultsV1 {
    view_identity: Option<[u8; 32]>,
    entries: Box<[ProductionSemanticMatrixBridgeResultV1]>,
    resources: SemanticSsaAuxiliaryResourcesV1,
}

impl DefinedMatrixResultsV1 {
    pub(super) fn derive(
        semantic: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &SemanticExpandedRootV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mismatch = || ProductionSemanticSsaErrorV1::ReplayMismatch;
        if !expansion
            .root(view.root())
            .is_some_and(|expected| std::ptr::eq(expected, view))
        {
            return Err(mismatch());
        }
        if !semantic.functions().iter().any(|function| matches!(function.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record)) if record.provenance().root() == view.root())) {
            return Ok(Self::default());
        }
        let bindings = expansion
            .defined_capability_bindings(semantic)
            .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
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
        // Logical peak includes the checked occurrence roster and its nested
        // operands, temporary return index, and growing result rows.
        for binding in &bindings {
            let mut bytes = size_of_val(binding)
                .checked_add(size_of_val(binding.arguments()))
                .and_then(|bytes| bytes.checked_add(size_of_val(binding.callee_arguments())))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            for place in binding
                .arguments()
                .iter()
                .filter_map(|operand| match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
                    SemanticOperandV1::Constant(_) => None,
                })
                .chain(std::iter::once(binding.destination()))
            {
                bytes = bytes
                    .checked_add(size_of_val(place.projections()))
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            }
            let words = bytes
                .div_ceil(size_of::<usize>())
                .checked_mul(2)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            charge(binding.arguments().len() + 1, words)?;
        }
        let mut returns =
            BTreeMap::<SemanticCallInstanceIdV1, Option<(SemanticBlockIdV1, u32)>>::new();
        for (block, origin) in view.block_origins().iter().enumerate() {
            charge(origin.statements().len() + 1, 0)?;
            for (statement, marker) in origin.statements().iter().enumerate() {
                if let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = marker {
                    charge(1, 16)?;
                    returns
                        .entry(*callee)
                        .and_modify(|site| *site = None)
                        .or_insert(Some((
                            SemanticBlockIdV1::from_index(block as u32),
                            statement as u32,
                        )));
                }
            }
        }
        let mut entries = Vec::new();
        for binding in bindings {
            charge(1, 0)?;
            let SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record) =
                binding.contract()
            else {
                continue;
            };
            if binding.root() != view.root() {
                continue;
            }
            if binding.callee_arguments().len() != 1 || binding.arguments().len() != 1 {
                return Err(mismatch());
            }
            let mut bridge = None;
            for (index, instance) in view.instances().iter().enumerate() {
                charge(1, 0)?;
                if instance.parent() != Some(binding.callee_instance()) {
                    continue;
                }
                let origin = view
                    .block_origins()
                    .get(instance.block_start() as usize)
                    .ok_or_else(mismatch)?;
                if instance.function() != record.bridge().function()
                    || instance.function_identity() != record.bridge().source_identity()
                    || origin.instance().index() as usize != index
                    || bridge.replace(origin.instance()).is_some()
                {
                    return Err(mismatch());
                }
            }
            let bridge = bridge.ok_or_else(mismatch)?;
            let &(block, statement) = returns
                .get(&bridge)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            let &(getter_block, getter_statement) = returns
                .get(&binding.callee_instance())
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            let transfer = assignment(view.body(), block, statement)?;
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) = transfer.value().kind()
            else {
                return Err(mismatch());
            };
            let return_origin = view
                .local_origins()
                .get(place.local().index() as usize)
                .ok_or_else(mismatch)?;
            let original = semantic
                .functions()
                .get(return_origin.function().index() as usize)
                .ok_or_else(mismatch)?;
            if return_origin.instance() != bridge
                || return_origin.function() != record.bridge().function()
                || original
                    .locals()
                    .get(return_origin.local().index() as usize)
                    .is_none_or(|local| local.role() != SemanticLocalRoleV1::Return)
                || !place.projections().is_empty()
                || place.ty() != record.types().matrix
                || transfer.destination().local() != binding.callee_return()
                || !transfer.destination().projections().is_empty()
                || transfer.destination().ty() != record.types().matrix
            {
                return Err(mismatch());
            }
            let mut current = None;
            for (body, origin) in view.body().blocks().iter().zip(view.block_origins()) {
                charge(1, 0)?;
                if origin.instance() != bridge {
                    continue;
                }
                if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() {
                    let destination = call.destination().ok_or_else(mismatch)?.place();
                    if call.callee() != record.current_callable()
                        || !call.arguments().is_empty()
                        || !destination.projections().is_empty()
                        || destination.ty() != record.types().unbranded_matrix
                        || current.replace(destination.local()).is_some()
                    {
                        return Err(mismatch());
                    }
                }
            }
            let getter_assignment = assignment(view.body(), getter_block, getter_statement)?;
            if getter_assignment.destination() != binding.destination()
                || !matches!(getter_assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(source))
                    if source.local() == binding.callee_return() && source.projections().is_empty() && source.ty() == record.types().matrix)
            {
                return Err(mismatch());
            }
            charge(128, 32)?;
            entries.push(ProductionSemanticMatrixBridgeResultV1 {
                getter: binding.callee_instance(),
                bridge,
                block,
                statement,
                getter_block,
                getter_statement,
                return_local: place.local(),
                destination: binding.callee_return(),
                receiver: binding.callee_arguments()[0],
                current: current.ok_or_else(mismatch)?,
                matrix: record.types().matrix,
            });
        }
        charge(
            entries
                .len()
                .checked_mul((usize::BITS - entries.len().leading_zeros()) as usize + 128)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            0,
        )?;
        entries.sort_by_key(|entry| (entry.block, entry.statement));
        let relation = Self {
            view_identity: Some(*view.identity()),
            entries: entries.into_boxed_slice(),
            resources,
        };
        relation.verify_view(view)?;
        Ok(relation)
    }

    pub(super) fn entries(&self) -> &[ProductionSemanticMatrixBridgeResultV1] {
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
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        self.verify_markers(view.body())?;
        let mut previous = None;
        for entry in &self.entries {
            let site = (entry.block, entry.statement);
            let source = view
                .block_origins()
                .get(entry.block.index() as usize)
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            if previous.is_some_and(|previous| previous >= site)
                || source.instance() != entry.bridge
                || source.statements().get(entry.statement as usize)
                    != Some(&SemanticExpandedStatementOriginV1::ReturnTransfer {
                        callee: entry.bridge,
                    })
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            previous = Some(site);
            let getter = view
                .block_origins()
                .get(entry.getter_block.index() as usize)
                .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
            if getter.instance() != entry.getter
                || getter.statements().get(entry.getter_statement as usize)
                    != Some(&SemanticExpandedStatementOriginV1::ReturnTransfer {
                        callee: entry.getter,
                    })
                || view
                    .instances()
                    .get(entry.bridge.index() as usize)
                    .is_none_or(|bridge| bridge.parent() != Some(entry.getter))
                || view
                    .local_origins()
                    .get(entry.receiver.index() as usize)
                    .is_none_or(|origin| origin.instance() != entry.getter)
                || view
                    .local_origins()
                    .get(entry.current.index() as usize)
                    .is_none_or(|origin| origin.instance() != entry.bridge)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn verify_markers(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let mut previous = None;
        for entry in &self.entries {
            let site = (entry.block, entry.statement);
            if previous.is_some_and(|previous| previous >= site) {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            previous = Some(site);
            let value = assignment(function, entry.block, entry.statement)?;
            if value.destination().local() != entry.destination
                || !value.destination().projections().is_empty()
                || value.destination().ty() != entry.matrix
                || !matches!(value.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                    if place.local() == entry.return_local && place.projections().is_empty() && place.ty() == entry.matrix)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            let getter_value = assignment(function, entry.getter_block, entry.getter_statement)?;
            if !matches!(getter_value.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                if place.local() == entry.destination && place.projections().is_empty() && place.ty() == entry.matrix)
            {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
        }
        Ok(())
    }

    /// Adds a checked result immediately before its original Move/Define/Kill
    /// events, never in an ambient entry roster and never in place of that Move.
    pub(super) fn append_result_events(
        &self,
        block: u32,
        statement: u32,
        events: &mut Vec<SsaEventV1>,
    ) {
        if let Ok(index) = self.entries.binary_search_by_key(
            &(SemanticBlockIdV1::from_index(block), statement),
            |entry| (entry.block, entry.statement),
        ) {
            let entry = self.entries[index];
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(
                entry.receiver.index(),
            )));
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(entry.current.index())));
            events.push(SsaEventV1::Define(SsaVariableIdV1::new(
                entry.return_local.index(),
            )));
        }
    }

    pub(super) fn hash_into(&self, digest: &mut Sha256) {
        if self.entries.is_empty() {
            return;
        }
        digest.update(b"fe2o3.execution-defined-matrix-results.v1\0");
        digest.update(
            self.view_identity
                .expect("derived relation has an exact view"),
        );
        digest.update((self.entries.len() as u64).to_le_bytes());
        for entry in &self.entries {
            for index in [
                entry.getter.index(),
                entry.bridge.index(),
                entry.block.index(),
                entry.statement,
                entry.getter_block.index(),
                entry.getter_statement,
                entry.return_local.index(),
                entry.destination.index(),
                entry.receiver.index(),
                entry.current.index(),
                entry.matrix.index(),
            ] {
                digest.update(index.to_le_bytes());
            }
        }
    }
}

fn assignment(
    function: &SemanticFunctionDeclV1,
    block: SemanticBlockIdV1,
    statement: u32,
) -> Result<&SemanticAssignmentV1, ProductionSemanticSsaErrorV1> {
    match function
        .blocks()
        .get(block.index() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .map(|statement| statement.kind())
    {
        Some(SemanticStatementKindV1::Assign(assignment)) => Ok(assignment),
        _ => Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
    }
}

#[cfg(test)]
#[path = "defined_matrix_results/tests.rs"]
pub(super) mod tests;
