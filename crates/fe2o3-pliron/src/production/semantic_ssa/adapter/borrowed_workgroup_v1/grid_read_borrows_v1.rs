//! Original read sinks for one authenticated Grid construction occurrence.
//! The existing complete borrow-component audit remains the acceptance owner.
use crate::production::semantic_ssa::{ProductionSemanticSsaErrorV1, guarded_grid_results};
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedRootV1, SemanticExpandedStatementOriginV1,
};
use std::collections::BTreeMap;
#[path = "grid_read_borrows_v1/field_reader.rs"]
mod field_reader;
#[path = "grid_read_borrows_v1/primitive_read.rs"]
mod primitive_read;
#[cfg(test)]
#[path = "grid_read_borrows_v1/inventory_probe.rs"]
pub(super) mod inventory_probe;

type Site = (u32, u32);
type Error = ProductionSemanticSsaErrorV1;
fn mismatch() -> Error {
    Error::ReplayMismatch
}
fn lookup_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

#[derive(Default)]
pub(super) struct Facts<'a> {
    pub pairs: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    reads: BTreeMap<Site, (&'a SemanticStatementKindV1, u32)>,
    roots: BTreeMap<Site, (u32, SemanticTypeIdV1)>,
}

impl<'a> Facts<'a> {
    pub fn new(
        semantic: &AdmittedInertSemanticMirV1,
        view: &'a SemanticExpandedRootV1,
        bindings: &[SemanticExpandedDefinedCapabilityV1],
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let mut result = Self::default();
        let mut allocated = false;
        for binding in bindings {
            charge(1)?;
            if binding.root() != view.root() {
                continue;
            }
            let SemanticDefinedCapabilityContractV1::GuardedGridLeader(record) = binding.contract()
            else {
                continue;
            };
            if !allocated {
                charge(9)?;
                allocated = true;
            }
            if binding.root_identity() != view.identity()
                || binding.caller_function() != record.source().caller
                || binding.call_block() != record.source().call_block
                || record.provenance().root() != view.root()
            {
                return Err(mismatch());
            }
            // The admitted contract authenticates the complete guarded body and
            // the caller's exact Option/receiver transfer. No ordinal is trusted.
            result.register(
                semantic,
                view,
                binding.caller_instance(),
                binding.callee_instance(),
                Some(record.types().grid_reference),
                charge,
            )?;
            let getter = unique_child(
                view,
                binding.caller_instance(),
                record.source().grid_getter,
                Some(record.source().grid_call_block),
                charge,
            )?
            .ok_or_else(mismatch)?;
            let current = unique_child(view, getter, record.source().grid_current, None, charge)?
                .ok_or_else(mismatch)?;
            if view.instances()[getter.index() as usize].function_identity()
                != record.grid_getter().source
                || view.instances()[current.index() as usize].function_identity()
                    != record.grid_current().source
            {
                return Err(mismatch());
            }
            // Only direct children of this exact original construction occurrence
            // are eligible. No type-wide or callee-global reader trust exists.
            for (index, frame) in view.instances().iter().enumerate() {
                charge(1)?;
                if frame.parent() == Some(current) {
                    result.register(
                        semantic,
                        view,
                        current,
                        instance_id(view, index)?,
                        None,
                        charge,
                    )?;
                }
            }
        }
        Ok(result)
    }

    fn register(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
        view: &'a SemanticExpandedRootV1,
        caller: SemanticCallInstanceIdV1,
        callee: SemanticCallInstanceIdV1,
        guarded_reference: Option<SemanticTypeIdV1>,
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        charge(24)?;
        let frame = view
            .instances()
            .get(callee.index() as usize)
            .ok_or_else(mismatch)?;
        let caller_frame = view
            .instances()
            .get(caller.index() as usize)
            .ok_or_else(mismatch)?;
        if frame.parent() != Some(caller) {
            return Err(mismatch());
        }
        let function = semantic
            .functions()
            .get(frame.function().index() as usize)
            .ok_or_else(mismatch)?;
        let caller_function = semantic
            .functions()
            .get(caller_frame.function().index() as usize)
            .ok_or_else(mismatch)?;
        if frame.function_identity() != function.identity()
            || caller_frame.function_identity() != caller_function.identity()
        {
            return Err(mismatch());
        }
        let abi = function.abi();
        let ([reference], [argument]) = (abi.source_input_types(), abi.arguments()) else {
            return Ok(());
        };
        if function.role() != SemanticFunctionRoleV1::InternalHelper
            || function.export().is_some()
            || abi.canon_abi() != SemanticCanonAbiV1::Rust
            || abi.extern_abi() != SemanticExternAbiV1::Rust
            || abi.can_unwind()
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
            || abi.fixed_count() != 1
            || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::SharedBorrow]
            || argument.ty() != *reference
            || argument.role() != SemanticAbiArgumentRoleV1::Source
            || !matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || abi.return_value().ty() != abi.source_output_type()
            || abi.return_value().adjusted().is_some()
            || abi.return_value().pointee_override().is_some()
            || guarded_reference.is_some_and(|expected| expected != *reference)
        {
            return Ok(());
        }
        let Some(owned) = primitive_read::shared_snapshot(semantic.types(), *reference, charge)?
        else {
            return Ok(());
        };
        if guarded_reference.is_none()
            && !field_reader::plain_snapshot(semantic.types(), abi.source_output_type(), charge)?
        {
            return Ok(());
        }
        let call_block = frame.call_block().ok_or_else(mismatch)?;
        let source_block = caller_function
            .blocks()
            .get(call_block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
            return Err(mismatch());
        };
        if semantic.callables().get(call.callee().index() as usize)
            != Some(&SemanticCallableDeclV1::defined(frame.function()))
            || call.unwind() != SemanticUnwindActionV1::Unreachable
            || call.destination().is_none_or(|d| {
                d.edge().role() != SemanticEdgeRoleV1::CallReturn
                    || !d.place().projections().is_empty()
                    || d.place().ty() != abi.source_output_type()
            })
        {
            return Err(mismatch());
        }
        let [operand] = call.arguments() else {
            return Err(mismatch());
        };
        let source_reference = match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
                if p.ty() == *reference && p.projections().is_empty() =>
            {
                p.local()
            }
            _ => return Ok(()),
        };
        let Some((borrow_site, owner)) =
            original_borrow(caller_function, source_reference, *reference, owned, charge)?
        else {
            return Ok(());
        };
        if !guarded_grid_results::initialized_shared_owner(
            caller_function,
            owner,
            &mut |work, words| charge(work.checked_add(words).ok_or(Error::ResourceOverflow)?),
        )? {
            return Ok(());
        }
        let mut receiver = None;
        for (index, local) in function.locals().iter().enumerate() {
            charge(1)?;
            if local.role() == SemanticLocalRoleV1::Argument(0) {
                if local.ty() != *reference
                    || receiver
                        .replace(SemanticLocalIdV1::from_index(index as u32))
                        .is_some()
                {
                    return Err(mismatch());
                }
            }
        }
        let receiver = receiver.ok_or_else(mismatch)?;
        let mut root = None;
        let mut reads = 0_usize;
        // Sinks are original Source statements and their actual mapped receiver,
        // not equal-looking expressions. Other uses retain the ordinary audit.
        for (bi, origin) in view.block_origins().iter().enumerate() {
            charge(1)?;
            if origin.instance() != callee && origin.instance() != caller {
                continue;
            }
            let block = view.body().blocks().get(bi).ok_or_else(mismatch)?;
            for (si, marker) in origin.statements().iter().enumerate() {
                charge(1)?;
                let SemanticExpandedStatementOriginV1::Source { statement } = marker else {
                    continue;
                };
                let kind = block.statements().get(si).ok_or_else(mismatch)?.kind();
                let SemanticStatementKindV1::Assign(a) = kind else {
                    continue;
                };
                let site = (bi as u32, si as u32);
                if origin.instance() == caller
                    && (origin.block().index(), *statement) == borrow_site
                {
                    if root.is_some() {
                        return Err(mismatch());
                    }
                    let SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    } = a.value().kind()
                    else {
                        return Err(mismatch());
                    };
                    if a.destination().ty() != *reference
                        || !a.destination().projections().is_empty()
                        || a.value().result_type() != *reference
                        || place.ty() != owned
                        || !place.projections().is_empty()
                        || !mapped_local(view, place.local(), caller, owner)
                        || !mapped_local(view, a.destination().local(), caller, source_reference)
                    {
                        return Err(mismatch());
                    }
                    root = Some((site, (place.local().index(), owned)));
                }
                if origin.instance() != callee {
                    continue;
                }
                let Some(source_statement) = function
                    .blocks()
                    .get(origin.block().index() as usize)
                    .and_then(|b| b.statements().get(*statement as usize))
                else {
                    return Err(mismatch());
                };
                let SemanticStatementKindV1::Assign(original) = source_statement.kind() else {
                    continue;
                };
                if !primitive_read::copied_field(
                    original,
                    receiver,
                    owned,
                    semantic.types(),
                    charge,
                )? {
                    continue;
                }
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) = a.value().kind() else {
                    return Err(mismatch());
                };
                if !mapped_local(view, p.local(), callee, receiver)
                    || !primitive_read::copied_field(a, p.local(), owned, semantic.types(), charge)?
                    || !matches!(original.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(q))
                        if p.projections() == q.projections() && p.ty() == q.ty())
                    || !mapped_local(
                        view,
                        a.destination().local(),
                        callee,
                        original.destination().local(),
                    )
                {
                    return Err(mismatch());
                }
                if self.reads.len() == 256 {
                    return Err(Error::ResourceOverflow);
                }
                charge(12 + lookup_work(self.reads.len()))?;
                if self.reads.insert(site, (kind, p.local().index())).is_some() {
                    return Err(mismatch());
                }
                reads += 1;
            }
        }
        if reads == 0 {
            return Ok(());
        }
        let (site, root) = root.ok_or_else(mismatch)?;
        if self.roots.len() == 64 {
            return Err(Error::ResourceOverflow);
        }
        charge(12 + lookup_work(self.roots.len()) + 12 + lookup_work(self.pairs.len()))?;
        if self
            .roots
            .insert(site, root)
            .is_some_and(|previous| previous != root)
            || self
                .pairs
                .insert(*reference, owned)
                .is_some_and(|previous| previous != owned)
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub fn captured(
        &self,
        site: Site,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Option<u32>, Error> {
        if self.reads.is_empty() {
            return Ok(None);
        }
        charge(lookup_work(self.reads.len()))?;
        Ok(self
            .reads
            .get(&site)
            .filter(|(expected, _)| std::ptr::eq(*expected, kind))
            .map(|(_, local)| *local))
    }
    pub fn allows_root(
        &self,
        site: Site,
        local: u32,
        owned: SemanticTypeIdV1,
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<bool, Error> {
        charge(lookup_work(self.roots.len()))?;
        Ok(self.roots.get(&site) == Some(&(local, owned)))
    }
}

fn mapped_local(
    view: &SemanticExpandedRootV1,
    expanded: SemanticLocalIdV1,
    instance: SemanticCallInstanceIdV1,
    original: SemanticLocalIdV1,
) -> bool {
    view.local_origins()
        .get(expanded.index() as usize)
        .is_some_and(|o| {
            o.instance() == instance
                && o.local() == original
                && view
                    .instances()
                    .get(instance.index() as usize)
                    .is_some_and(|f| f.function() == o.function())
        })
}
fn instance_id(
    view: &SemanticExpandedRootV1,
    index: usize,
) -> Result<SemanticCallInstanceIdV1, Error> {
    let frame = view.instances().get(index).ok_or_else(mismatch)?;
    let origin = view
        .block_origins()
        .get(frame.block_start() as usize)
        .ok_or_else(mismatch)?;
    if origin.instance().index() as usize != index || origin.function() != frame.function() {
        return Err(mismatch());
    }
    Ok(origin.instance())
}
fn unique_child(
    view: &SemanticExpandedRootV1,
    parent: SemanticCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    block: Option<SemanticBlockIdV1>,
    charge: &mut impl FnMut(usize) -> Result<(), Error>,
) -> Result<Option<SemanticCallInstanceIdV1>, Error> {
    let mut result = None;
    for (index, frame) in view.instances().iter().enumerate() {
        charge(1)?;
        if frame.parent() == Some(parent)
            && frame.function() == function
            && block.is_none_or(|b| frame.call_block() == Some(b))
        {
            if result.replace(instance_id(view, index)?).is_some() {
                return Err(mismatch());
            }
        }
    }
    Ok(result)
}
fn original_borrow(
    function: &SemanticFunctionDeclV1,
    reference_local: SemanticLocalIdV1,
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
    charge: &mut impl FnMut(usize) -> Result<(), Error>,
) -> Result<Option<(Site, SemanticLocalIdV1)>, Error> {
    let mut found = None;
    for (bi, block) in function.blocks().iter().enumerate() {
        charge(1)?;
        for (si, statement) in block.statements().iter().enumerate() {
            charge(1)?;
            let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                continue;
            };
            if a.destination().local() != reference_local {
                continue;
            }
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                return Ok(None);
            };
            if found.is_some()
                || !a.destination().projections().is_empty()
                || a.destination().ty() != reference
                || a.value().result_type() != reference
                || place.ty() != owned
                || !place.projections().is_empty()
            {
                return Ok(None);
            }
            found = Some(((bi as u32, si as u32), place.local()));
        }
    }
    Ok(found)
}
