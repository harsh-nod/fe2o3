use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

mod narrow;
mod cursor;
mod bound_reads;
pub(in super::super) use cursor::CaptureCursor;

#[derive(Default)]
pub(super) struct ScopedCaptures<'a> {
    sites: BTreeMap<SemanticTransparentBorrowSiteV1, Capture<'a>>,
    matrix_leaves: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    policy_leaves: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    wrapper_types: BTreeSet<SemanticTypeIdV1>,
    work: usize,
}

struct Capture<'a> {
    assignment: &'a SemanticAssignmentV1,
    locals: [u32; 2],
    len: usize,
}

// Logical key-operation accounting, not a physical BTree fanout bound.
fn key_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

fn mismatch() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::ReplayMismatch
}

impl<'a> ScopedCaptures<'a> {
    pub(super) fn cursor(
        &self,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<CaptureCursor<'_, 'a>, ProductionSemanticSsaErrorV1> {
        CaptureCursor::new(&self.sites, charge)
    }

    pub(super) fn work_units(&self) -> usize {
        self.work
    }

    pub(super) fn matrix_leaves(&self) -> &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1> {
        &self.matrix_leaves
    }

    pub(super) fn policy_leaves(&self) -> &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1> {
        &self.policy_leaves
    }

    pub(super) fn wrapper_types(&self) -> &BTreeSet<SemanticTypeIdV1> {
        &self.wrapper_types
    }

    // Called only for this root's Bind/Narrow during the existing inventory
    // pass. Irrelevant records retain the original 32*N constructor boundary.
    pub(super) fn register(
        &mut self,
        source: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &'a SemanticExpandedRootV1,
        binding: &SemanticExpandedDefinedCapabilityV1,
        pairs: &mut BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
        limit: usize,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.charge(32, limit)?;
        if expansion.source_semantic_sha256() != source.semantic_sha256().as_bytes()
            || !expansion
                .root(view.root())
                .is_some_and(|actual| std::ptr::eq(actual, view))
            || binding.root() != view.root()
            || binding.root_identity() != view.identity()
            || binding.expansion_identity() != expansion.identity()
        {
            return Err(mismatch());
        }
        let (function, references, count, output, fields) = match binding.contract() {
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => {
                let t = record.types();
                (
                    record.function(),
                    [
                        (t.matrix_reference, t.matrix),
                        (t.policy_reference, t.capability),
                    ],
                    2,
                    t.bound,
                    4,
                )
            }
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                let t = record.types();
                let pair = (t.bound_reference, t.bind.bound);
                (record.function(), [pair, pair], 1, t.narrowed, 2)
            }
            _ => return Err(mismatch()),
        };
        let references = &references[..count];
        self.charge(references.len() * 16, limit)?;
        let original = source
            .functions()
            .get(function.index() as usize)
            .ok_or_else(mismatch)?;
        if original.defined_capability_contract().copied() != Some(binding.contract())
            || binding.arguments().len() != references.len()
            || binding.callee_arguments().len() != references.len()
            || original.abi().source_argument_ownership().len() != references.len()
            || !original
                .abi()
                .source_argument_ownership()
                .iter()
                .all(|o| *o == SemanticSourceArgumentOwnershipV1::SharedBorrow)
            || !original
                .abi()
                .source_input_types()
                .iter()
                .copied()
                .eq(references.iter().map(|(r, _)| *r))
            || original.abi().source_output_type() != output
        {
            return Err(mismatch());
        }
        let block = match binding.contract() {
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                self.narrow(source, view, binding, record, limit)?
            }
            _ => binding.expanded_entry_block().index(),
        };
        let site = SemanticTransparentBorrowSiteV1 {
            block,
            statement: 0,
        };
        let assignment = original_assignment(view, site)?;
        let origin = &view.block_origins()[block as usize];
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Err(mismatch());
        };
        if origin.function() != function
            || origin.instance() != binding.callee_instance()
            || assignment.destination().local() != binding.callee_return()
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != output
            || assignment.value().result_type() != output
            || aggregate.kind() != &SemanticAggregateKindV1::Aggregate
            || aggregate.operands().len() != fields
        {
            return Err(mismatch());
        }
        let mut locals = [0; 2];
        for (index, (&local, &(reference, owned))) in binding
            .callee_arguments()
            .iter()
            .zip(references)
            .enumerate()
        {
            let origin = view
                .local_origins()
                .get(local.index() as usize)
                .ok_or_else(mismatch)?;
            if origin.function() != function
                || origin.instance() != binding.callee_instance()
                || view
                    .body()
                    .locals()
                    .get(local.index() as usize)
                    .is_none_or(|decl| decl.ty() != reference)
                || binding.arguments()[index].ty() != reference
                || !matches!(&aggregate.operands()[index], SemanticOperandV1::Copy(place)
                    if place.local() == local && place.ty() == reference && place.projections().is_empty())
                || !shared(source.types(), reference, owned)
            {
                return Err(mismatch());
            }
            self.charge(key_work(pairs.len()) + 2, limit)?;
            if pairs
                .insert(reference, owned)
                .is_some_and(|old| old != owned)
            {
                return Err(mismatch());
            }
            locals[index] = local.index();
        }
        self.insert(site, assignment, locals, references.len(), limit)?;
        // Only the replay-checked Bind seeds ordinary carrier traversal. The
        // branded constructors keep their exact occurrence-specific handling.
        if let SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) = binding.contract() {
            let t = record.types();
            self.charge(key_work(self.matrix_leaves.len()) + 4, limit)?;
            if self.matrix_leaves.insert(t.matrix_reference, t.matrix)
                .is_some_and(|old| old != t.matrix)
            {
                return Err(mismatch());
            }
            // The same checked Bind supplies the policy's exact shared pair.
            // No policy is inferred from a name, layout or ambient ZST.
            self.charge(key_work(self.policy_leaves.len()) + 4, limit)?;
            if self.policy_leaves.insert(t.policy_reference, t.capability)
                .is_some_and(|old| old != t.capability)
            {
                return Err(mismatch());
            }
        }
        self.charge(key_work(self.wrapper_types.len()) + 2, limit)?;
        self.wrapper_types.insert(output);
        Ok(())
    }

    fn insert(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        assignment: &'a SemanticAssignmentV1,
        locals: [u32; 2],
        len: usize,
        limit: usize,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.charge(key_work(self.sites.len()) + 8, limit)?;
        if self
            .sites
            .insert(
                site,
                Capture {
                    assignment,
                    locals,
                    len,
                },
            )
            .is_some()
        {
            return Err(mismatch());
        }
        Ok(())
    }

    fn charge(&mut self, amount: usize, limit: usize) -> Result<(), ProductionSemanticSsaErrorV1> {
        let required = self
            .work
            .checked_add(amount)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if required > limit {
            return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required,
                limit,
            });
        }
        self.work = required;
        Ok(())
    }

    pub(super) fn captured(
        &self,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<Option<&[u32]>, ProductionSemanticSsaErrorV1> {
        if self.sites.is_empty() {
            return Ok(None);
        }
        charge(key_work(self.sites.len()))?;
        let Some(capture) = self.sites.get(&site) else {
            return Ok(None);
        };
        Ok(matches!(kind, SemanticStatementKindV1::Assign(actual) if std::ptr::eq(actual, capture.assignment))
            .then_some(&capture.locals[..capture.len]))
    }
}

fn original_assignment(
    view: &SemanticExpandedRootV1,
    site: SemanticTransparentBorrowSiteV1,
) -> Result<&SemanticAssignmentV1, ProductionSemanticSsaErrorV1> {
    let origin = view
        .block_origins()
        .get(site.block as usize)
        .ok_or_else(mismatch)?;
    if origin.statements().get(site.statement as usize)
        != Some(&SemanticExpandedStatementOriginV1::Source {
            statement: site.statement,
        })
    {
        return Err(mismatch());
    }
    match view
        .body()
        .blocks()
        .get(site.block as usize)
        .and_then(|b| b.statements().get(site.statement as usize))
        .map(|s| s.kind())
    {
        Some(SemanticStatementKindV1::Assign(assignment)) => Ok(assignment),
        _ => Err(mismatch()),
    }
}

fn shared(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
) -> bool {
    reference != owned
        && matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(p)) if p.pointee() == owned && p.kind() == SemanticPointerKindV1::Reference
            && p.mutability() == SemanticMutabilityV1::Immutable && p.address_space() == 0
            && p.pointer_width_bits() == 64 && p.metadata() == SemanticPointerMetadataV1::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("scoped_capture_tests.rs");
}
