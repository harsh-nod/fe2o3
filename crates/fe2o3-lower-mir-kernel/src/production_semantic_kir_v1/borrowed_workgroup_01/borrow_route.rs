use super::super::capability_ssa_graph_01::shared_source_path::{check_path, shared_pointee};
use super::*;

const MAX_PATH: usize = 16;

pub(super) struct BorrowRoute {
    pub(super) path: Vec<SemanticProjectionV1>,
    pub(super) storage_loan: bool,
}

// Charge before constructing the vector or traversing either bounded path.
// Logical words/visits, not a claim about physical allocator or stack bytes.
pub(super) fn route_work(source_len: usize, tail_len: usize) -> Result<usize> {
    let combined = source_len.checked_add(tail_len).filter(|n| *n <= MAX_PATH);
    match combined {
        Some(n) => Ok(12 + 4 * n),
        None => Err(reject(
            "borrowed Workgroup source path is cyclic or too deep",
        )),
    }
}

pub(super) fn borrow_route(
    types: &[SemanticTypeDeclV1],
    source_type: SemanticTypeIdV1,
    source: &SemanticPlaceV1,
    reference: SemanticTypeIdV1,
    tail: &[SemanticProjectionV1],
) -> Result<BorrowRoute> {
    let _ = route_work(source.projections().len(), tail.len())?;
    let pointee = shared_pointee(types, reference)
        .ok_or_else(|| reject("Workgroup carrier borrow lost its exact shared reference edge"))?;
    if pointee != source.ty() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    check_path(types, source_type, source.projections(), source.ty())
        .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let rest = if let Some((first, rest)) = tail.split_first() {
        if first.kind() != SemanticProjectionKindV1::Dereference || first.result_type() != pointee {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let target = tail.last().unwrap().result_type();
        check_path(types, reference, tail, target)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        rest
    } else {
        &[]
    };
    let mut path = Vec::new();
    path.try_reserve_exact(source.projections().len() + rest.len())
        .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
        })?;
    path.extend_from_slice(source.projections());
    path.extend_from_slice(rest);
    // &slot / &carrier.field borrows that storage even when its type is itself
    // a reference. A dereference borrows the pointee instead; its earlier loan
    // is retained by recursive source resolution, not recreated here.
    let storage_loan = !source
        .projections()
        .iter()
        .any(|p| p.kind() == SemanticProjectionKindV1::Dereference);
    Ok(BorrowRoute { path, storage_loan })
}

#[cfg(test)]
#[path = "borrow_route_tests.rs"]
mod tests;
