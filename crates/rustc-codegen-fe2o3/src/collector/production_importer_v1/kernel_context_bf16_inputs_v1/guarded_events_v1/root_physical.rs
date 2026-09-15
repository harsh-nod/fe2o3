//! Same-reference transport to an actual root ABI input, not allocation proof.
use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
    ProductionSemanticSsaValueOriginV1 as Origin,
};

#[derive(Clone, Copy)]
pub(crate) struct RootPhysicalV1<'a> {
    owner: &'a fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    query: ProductionSemanticSsaSourceQueryV1<'a>,
    operand: &'a SemanticOperandV1,
    original: reference_bindings::SsaUse,
    entry_variable: u32,
    entry_value: SsaValueV1,
    argument: u32,
}

impl RootPhysicalV1<'_> {
    pub(crate) fn belongs_to(&self, row: ProductionGlobalBf16SourceRowV1<'_, '_>) -> bool {
        std::ptr::eq(self.owner, row.owner())
            && std::ptr::eq(self.query.function(), row.view().body())
            && self.query.root() == row.view().root()
            && std::ptr::eq(self.operand, row.physical().operand())
            && self.original.site == row.physical().site()
            && self.original.ty == row.physical().operand().ty()
    }
    pub(crate) const fn root_argument(&self) -> u32 { self.argument }
    pub(crate) const fn root_local(&self) -> u32 { self.entry_variable }
    pub(crate) const fn root_value(&self) -> SsaValueV1 { self.entry_value }
    pub(crate) const fn physical_local(&self) -> u32 { self.original.variable }
    pub(crate) const fn physical_type(&self) -> SemanticTypeIdV1 { self.original.ty }
}

pub(super) fn project<'a>(
    row: ProductionGlobalBf16SourceRowV1<'_, 'a>,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<RootPhysicalV1<'a>> {
    charge(1 + std::mem::size_of::<RootPhysicalV1<'_>>().div_ceil(std::mem::size_of::<usize>()))?;
    let roots = row.owner().execution_expansion().roots().len();
    charge(4 * (usize::BITS as usize - roots.leading_zeros() as usize + 1))?;
    let query = row.owner().source_query_for_root(row.view().root(), row.view().body())
        .map_err(|_| E::CorrespondenceMismatch)?;
    let slice = physical_extent_slice(row.owner().source_semantic().types(),
        row.physical().operand().ty(), row.contract().types().element, charge)?;
    let original = reference_bindings::reference_use(
        row.owner().source_semantic().types(), &query, row.physical(), slice,
        SemanticPointerMetadataV1::SliceLength, charge,
    )?;
    let (entry_variable, entry_value, argument) = trace_entry(
        &query, original.variable, original.value, original.ty, slice, charge,
    )?;
    Ok(RootPhysicalV1 { owner: row.owner(), query, operand: row.physical().operand(),
        original, entry_variable, entry_value, argument })
}

// Follow only the existing indexed definitions. No reconstruction, new solver,
// phi guess, or local-provenance fallback. A merge/returned/transformed pointer
// needs a separate exact relation and stays unsupported in this finite path.
fn trace_entry(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    mut variable: u32,
    mut value: SsaValueV1,
    physical: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<(u32, SsaValueV1, u32)> {
    let body = query.function();
    let mut remaining = query.plan().plan().definition_count().checked_add(1)
        .ok_or(E::CorrespondenceMismatch)?;
    loop {
        charge(1)?;
        remaining = remaining.checked_sub(1).ok_or(E::CorrespondenceMismatch)?;
        let SsaValueV1::Definition(definition) = value else {
            return Err(E::CorrespondenceMismatch);
        };
        let (actual, origin) = with_query_charge(charge, |f| query.definition_origin(definition, &mut || f()))?;
        if actual.get() != variable
            || body.locals().get(variable as usize).is_none_or(|local| local.ty() != physical)
        {
            return Err(E::CorrespondenceMismatch);
        }
        match origin {
            Origin::Entry { .. } => {
                let SemanticLocalRoleV1::Argument(argument) = body.locals()[variable as usize].role() else {
                    return Err(E::CorrespondenceMismatch);
                };
                if body.abi().source_input_types().get(argument as usize) != Some(&physical)
                    || body.abi().source_argument_ownership().get(argument as usize)
                        != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
                {
                    return Err(E::CorrespondenceMismatch);
                }
                return Ok((variable, value, argument));
            }
            Origin::Event { site, .. } => {
                let statement = site.statement().ok_or(E::CorrespondenceMismatch)?;
                let SemanticStatementKindV1::Assign(assignment) = body.blocks()
                    .get(site.block().index() as usize)
                    .and_then(|block| block.statements().get(statement as usize))
                    .ok_or(E::CorrespondenceMismatch)?.kind()
                else { return Err(E::CorrespondenceMismatch); };
                if assignment.destination().local().index() != variable
                    || !assignment.destination().projections().is_empty()
                    || assignment.destination().ty() != physical
                    || assignment.value().result_type() != physical
                {
                    return Err(E::CorrespondenceMismatch);
                }
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand) if operand.ty() == physical => {
                        let used = with_query_charge(charge, |f| query.operand_use(site, operand, &mut || f()))?;
                        variable = used.variable().get();
                        value = used.value();
                    }
                    SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place }
                        if place.ty() == pointee
                            && matches!(place.projections(), [projection]
                                if projection.kind() == SemanticProjectionKindV1::Dereference
                                && projection.result_type() == pointee)
                            && body.locals().get(place.local().index() as usize)
                                .is_some_and(|local| local.ty() == physical) =>
                    {
                        value = with_query_charge(charge, |f| query.borrow_place_use(site, place, &mut || f()))?;
                        variable = place.local().index();
                    }
                    _ => return Err(E::CorrespondenceMismatch),
                }
            }
            Origin::Edge { .. } | Origin::BlockArgument(_) => return Err(E::CorrespondenceMismatch),
        }
    }
}

fn with_query_charge<T>(
    charge: &mut dyn FnMut(usize) -> Result<()>,
    inspect: impl FnOnce(&mut dyn FnMut() -> bool) -> std::result::Result<T, QueryError>,
) -> Result<T> {
    let mut failure = None;
    let result = inspect(&mut || match charge(1) {
        Ok(()) => true,
        Err(error) => { failure = Some(error); false }
    });
    if let Some(error) = failure { return Err(error); }
    result.map_err(|_| E::CorrespondenceMismatch)
}

